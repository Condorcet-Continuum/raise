// FICHIER : src-tauri/src/workflow_engine/scheduler.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::{prelude::*, HashMap};
use crate::workflow_engine::{
    executor::WorkflowExecutor, state_machine::WorkflowStateMachine, ExecutionStatus,
    WorkflowDefinition, WorkflowInstance,
};

/// Le Scheduler orchestre le cycle de vie des instances de workflow.
/// Il est responsable de la persistance en base de données et du routage vers l'Exécuteur.
pub struct WorkflowScheduler {
    pub executor: WorkflowExecutor,
    pub definitions: HashMap<String, WorkflowDefinition>,
}

impl WorkflowScheduler {
    /// Crée un nouveau Scheduler en encapsulant l'Exécuteur configuré.
    pub fn new(executor: WorkflowExecutor) -> Self {
        Self {
            executor,
            definitions: HashMap::new(),
        }
    }

    /// Charge et compile un Mandat depuis la base de données, puis le met en cache.
    pub async fn load_mission(
        &mut self,
        mandate_id: &str,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<()> {
        tracing::info!("📥 Chargement de la mission (Mandat: {})", mandate_id);
        let workflow = WorkflowExecutor::load_and_prepare_workflow(manager, mandate_id).await?;
        self.definitions.insert(workflow.id.clone(), workflow);
        Ok(())
    }

    /// Instancie un nouveau Workflow et le sauvegarde immédiatement en base de données.
    pub async fn create_instance(
        &self,
        workflow_id: &str,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<WorkflowInstance> {
        let def = self.definitions.get(workflow_id).ok_or_else(|| {
            crate::utils::AppError::NotFound(format!("Définition '{}' introuvable", workflow_id))
        })?;

        let mut instance = WorkflowInstance {
            id: format!(
                "inst_{}_{}",
                workflow_id,
                chrono::Utc::now().timestamp_millis()
            ),
            workflow_id: def.id.clone(),
            status: ExecutionStatus::Pending,
            node_states: HashMap::new(),
            context: HashMap::new(),
            logs: vec![format!(
                "Création de l'instance pour le workflow {}",
                def.id
            )],
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };

        // Persistance initiale
        self.persist_instance(&mut instance, manager).await?;
        tracing::info!("✨ Nouvelle instance créée : {}", instance.id);

        Ok(instance)
    }

    /// Exécute un cycle (step) unique pour une instance et persiste l'état.
    pub async fn run_step(
        &self,
        instance: &mut WorkflowInstance,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<bool> {
        let def = self.definitions.get(&instance.workflow_id).ok_or_else(|| {
            crate::utils::AppError::NotFound(format!(
                "Définition '{}' non chargée",
                instance.workflow_id
            ))
        })?;

        let sm = WorkflowStateMachine::new(def);
        let runnable_nodes = sm.next_runnable_nodes(instance).await;

        if runnable_nodes.is_empty() {
            if instance.status == ExecutionStatus::Running {
                instance.status = ExecutionStatus::Completed;
                instance
                    .logs
                    .push("🏁 Exécution terminée avec succès.".into());
                self.persist_instance(instance, manager).await?;
            }
            return Ok(false);
        }

        instance.status = ExecutionStatus::Running;
        let mut progress_made = false;

        for node_id in runnable_nodes {
            if let Some(node) = def.nodes.iter().find(|n| n.id == node_id) {
                // 1. Délégation à l'Exécuteur (Strategy Pattern)
                let status = self
                    .executor
                    .execute_node(node, &mut instance.context)
                    .await?;

                // 2. Transition d'état en mémoire
                sm.transition(instance, &node_id, status)
                    .map_err(|e| crate::utils::AppError::from(e.to_string()))?;

                instance
                    .logs
                    .push(format!("⚙️ Nœud '{}' exécuté -> {:?}", node.name, status));
                progress_made = true;

                // 3. Gestion de la pause (HITL)
                if status == ExecutionStatus::Paused {
                    instance.status = ExecutionStatus::Paused;
                    instance
                        .logs
                        .push(format!("⏸️ Workflow en pause sur '{}'", node.name));
                    break;
                }

                // 4. Gestion de l'échec critique (Fail-Safe)
                if status == ExecutionStatus::Failed {
                    instance.status = ExecutionStatus::Failed;
                    instance
                        .logs
                        .push(format!("🚨 Échec critique sur '{}'. Arrêt.", node.name));
                    break;
                }
            }
        }

        if progress_made {
            self.persist_instance(instance, manager).await?;
        }

        Ok(progress_made)
    }

    /// Boucle de haut niveau : exécute le workflow de manière autonome jusqu'à la fin ou la pause.
    pub async fn execute_instance_loop(
        &self,
        instance_id: &str,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<ExecutionStatus> {
        let doc = manager
            .get_document("workflow_instances", instance_id)
            .await
            .map_err(|e| crate::utils::AppError::Database(e.to_string()))?
            .ok_or_else(|| {
                crate::utils::AppError::NotFound(format!("Instance {} introuvable", instance_id))
            })?;

        let mut instance: WorkflowInstance =
            serde_json::from_value(doc).map_err(crate::utils::AppError::Serialization)?;

        tracing::info!("🚀 Démarrage/Reprise boucle pour {}", instance.id);

        loop {
            let progress = self.run_step(&mut instance, manager).await?;
            if !progress {
                break;
            }
        }

        Ok(instance.status)
    }

    /// Débloque un nœud en pause (GateHitl) suite à une décision humaine.
    pub async fn resume_node(
        &self,
        instance_id: &str,
        node_id: &str,
        approved: bool,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<ExecutionStatus> {
        let doc = manager
            .get_document("workflow_instances", instance_id)
            .await
            .map_err(|e| crate::utils::AppError::Database(e.to_string()))?
            .ok_or_else(|| {
                crate::utils::AppError::NotFound(format!("Instance {} introuvable", instance_id))
            })?;

        let mut instance: WorkflowInstance =
            serde_json::from_value(doc).map_err(crate::utils::AppError::Serialization)?;

        let new_status = if approved {
            ExecutionStatus::Completed
        } else {
            ExecutionStatus::Failed
        };

        instance.node_states.insert(node_id.to_string(), new_status);
        instance.status = ExecutionStatus::Running;
        instance.logs.push(format!(
            "👤 Décision humaine pour '{}' : {:?}",
            node_id, new_status
        ));

        self.persist_instance(&mut instance, manager).await?;
        tracing::info!("💾 Reprise enregistrée pour {}", instance.id);

        Ok(instance.status)
    }

    /// Utilitaire interne pour sauvegarder l'instance dans le JSON-DB
    async fn persist_instance(
        &self,
        instance: &mut WorkflowInstance,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<()> {
        instance.updated_at = chrono::Utc::now().timestamp();
        let json_val =
            serde_json::to_value(&instance).map_err(crate::utils::AppError::Serialization)?;

        manager
            .insert_raw("workflow_instances", &json_val)
            .await
            .map_err(|e| {
                crate::utils::AppError::Database(format!("Échec persistance instance: {}", e))
            })?;

        Ok(())
    }
}

// =========================================================================
// TESTS UNITAIRES ET D'INTÉGRATION ULTRA-ROBUSTES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::orchestrator::AiOrchestrator;
    use crate::json_db::schema::registry::SchemaRegistry;
    use crate::json_db::schema::SchemaValidator;
    use crate::json_db::test_utils::{ensure_db_exists, init_test_env};
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::utils::{config::test_mocks, Arc, AsyncMutex};
    use crate::workflow_engine::{NodeType, WorkflowEdge, WorkflowNode};
    use serde_json::{json, Value}; // 🎯 FIX: Import explicite de serde_json::Value

    /// Prépare un environnement complet pour les tests du Scheduler (DB, Orchestrator, Executor)
    async fn setup_test_environment() -> (
        &'static crate::json_db::test_utils::TestEnv, // 🎯 FIX: Utilisation d'une référence statique propre
        CollectionsManager<'static>,
        WorkflowScheduler,
    ) {
        let env_val = init_test_env().await;
        // On rend l'environnement statique dès le départ pour éviter tout besoin de le cloner
        let env = Box::leak(Box::new(env_val));

        test_mocks::inject_mock_config();
        ensure_db_exists(&env.cfg, &env.space, &env.db).await;

        // Préparation du schéma de DB pour les instances
        let dest_schemas = env.cfg.db_schemas_root(&env.space, &env.db).join("v1");
        std::fs::create_dir_all(&dest_schemas).unwrap();
        let instance_schema =
            json!({ "$schema": "http://json-schema.org/draft-07/schema#", "type": "object" });
        std::fs::write(
            dest_schemas.join("workflow_instances.json"),
            instance_schema.to_string(),
        )
        .unwrap();

        let reg = SchemaRegistry::from_db(&env.cfg, &env.space, &env.db)
            .await
            .unwrap();
        let _ = SchemaValidator::compile_with_registry(&reg.uri("workflow_instances.json"), &reg)
            .unwrap();

        let manager = CollectionsManager::new(&env.storage, &env.space, &env.db);
        manager
            .create_collection(
                "workflow_instances",
                Some("workflow_instances.json".to_string()),
            )
            .await
            .unwrap();

        // Création du Moteur
        let orch = AiOrchestrator::new(ProjectModel::default(), None)
            .await
            .unwrap();
        let pm = Arc::new(PluginManager::new(&env.storage, None));
        let executor = WorkflowExecutor::new(Arc::new(AsyncMutex::new(orch)), pm);
        let scheduler = WorkflowScheduler::new(executor);

        (env, manager, scheduler)
    }

    /// Helper pour créer manuellement un DAG personnalisé pour les tests
    fn create_mock_workflow(
        id: &str,
        nodes: Vec<WorkflowNode>,
        edges: Vec<WorkflowEdge>,
    ) -> WorkflowDefinition {
        // 🎯 FIX: Récupération automatique du premier nœud comme entry
        let entry = nodes.first().map(|n| n.id.clone()).unwrap_or_default();
        WorkflowDefinition {
            id: id.to_string(),
            entry,
            nodes,
            edges,
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_scheduler_create_instance_and_persistence() {
        let (_env, manager, mut scheduler) = setup_test_environment().await;

        let def = create_mock_workflow("wf_empty", vec![], vec![]);
        scheduler.definitions.insert("wf_empty".to_string(), def);

        let instance = scheduler
            .create_instance("wf_empty", &manager)
            .await
            .expect("Échec création");

        assert_eq!(instance.workflow_id, "wf_empty");
        assert_eq!(instance.status, ExecutionStatus::Pending);

        let doc = manager
            .get_document("workflow_instances", &instance.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            doc["workflowId"], "wf_empty",
            "Le workflowId doit être persisté"
        );
        assert_eq!(doc["status"], "PENDING", "Le statut doit être persisté");
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_scheduler_missing_definition_fails_safely() {
        let (_env, manager, scheduler) = setup_test_environment().await;

        let result = scheduler.create_instance("wf_ghost", &manager).await;

        assert!(
            result.is_err(),
            "La création doit échouer si le workflow n'est pas chargé"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("introuvable"),
            "Le message d'erreur doit être explicite"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_scheduler_step_by_step_execution() {
        let (_env, manager, mut scheduler) = setup_test_environment().await;

        let n_start = WorkflowNode {
            id: "n1".into(),
            r#type: NodeType::End,
            name: "Start".into(),
            params: Value::Null,
        };
        let def = create_mock_workflow("wf_mini", vec![n_start], vec![]);
        scheduler.definitions.insert("wf_mini".to_string(), def);

        let mut instance = scheduler
            .create_instance("wf_mini", &manager)
            .await
            .unwrap();

        let progress = scheduler.run_step(&mut instance, &manager).await.unwrap();
        assert!(progress, "Le scheduler aurait dû faire un pas");

        // 🎯 FIX : Dès que le nœud End est exécuté, le moteur clôture intelligemment le workflow !
        assert_eq!(instance.status, ExecutionStatus::Completed);

        let progress_end = scheduler.run_step(&mut instance, &manager).await.unwrap();
        assert!(
            !progress_end,
            "Le scheduler ne devrait plus pouvoir avancer"
        );
        assert_eq!(instance.status, ExecutionStatus::Completed);
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_scheduler_hitl_lifecycle_approved() {
        let (_env, manager, mut scheduler) = setup_test_environment().await;

        let n_hitl = WorkflowNode {
            id: "hitl_1".into(),
            r#type: NodeType::GateHitl,
            name: "Validation".into(),
            params: Value::Null,
        };
        let n_end = WorkflowNode {
            id: "end_1".into(),
            r#type: NodeType::End,
            name: "Fin".into(),
            params: Value::Null,
        };

        // 🎯 FIX: Utilisation de `from` et `to` au lieu de `source` et `target`
        let edge = WorkflowEdge {
            from: "hitl_1".into(),
            to: "end_1".into(),
            condition: None,
        };

        let def = create_mock_workflow("wf_hitl", vec![n_hitl, n_end], vec![edge]);
        scheduler.definitions.insert("wf_hitl".to_string(), def);

        let instance = scheduler
            .create_instance("wf_hitl", &manager)
            .await
            .unwrap();

        let status1 = scheduler
            .execute_instance_loop(&instance.id, &manager)
            .await
            .unwrap();
        assert_eq!(
            status1,
            ExecutionStatus::Paused,
            "Le workflow doit s'interrompre sur le GateHitl"
        );

        let resume_status = scheduler
            .resume_node(&instance.id, "hitl_1", true, &manager)
            .await
            .unwrap();
        assert_eq!(
            resume_status,
            ExecutionStatus::Running,
            "La reprise doit remettre le statut à Running"
        );

        let status2 = scheduler
            .execute_instance_loop(&instance.id, &manager)
            .await
            .unwrap();
        assert_eq!(
            status2,
            ExecutionStatus::Completed,
            "Le workflow doit se terminer avec succès après approbation"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_scheduler_hitl_lifecycle_rejected() {
        let (_env, manager, mut scheduler) = setup_test_environment().await;

        let n_hitl = WorkflowNode {
            id: "hitl_2".into(),
            r#type: NodeType::GateHitl,
            name: "Validation".into(),
            params: Value::Null,
        };
        let n_end = WorkflowNode {
            id: "end_2".into(),
            r#type: NodeType::End,
            name: "Fin".into(),
            params: Value::Null,
        };

        // 🎯 FIX: Utilisation de `from` et `to`
        let edge = WorkflowEdge {
            from: "hitl_2".into(),
            to: "end_2".into(),
            condition: None,
        };

        let def = create_mock_workflow("wf_reject", vec![n_hitl, n_end], vec![edge]);
        scheduler.definitions.insert("wf_reject".to_string(), def);

        let instance = scheduler
            .create_instance("wf_reject", &manager)
            .await
            .unwrap();

        scheduler
            .execute_instance_loop(&instance.id, &manager)
            .await
            .unwrap();

        scheduler
            .resume_node(&instance.id, "hitl_2", false, &manager)
            .await
            .unwrap();

        let _ = scheduler
            .execute_instance_loop(&instance.id, &manager)
            .await
            .unwrap();

        let doc = manager
            .get_document("workflow_instances", &instance.id)
            .await
            .unwrap()
            .unwrap();
        let saved_instance: WorkflowInstance = serde_json::from_value(doc).unwrap();

        assert_eq!(
            saved_instance.node_states.get("hitl_2").unwrap(),
            &ExecutionStatus::Failed,
            "Le rejet humain doit marquer le nœud comme Failed"
        );
    }
}
