// FICHIER : src-tauri/src/workflow_engine/scheduler.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::{prelude::*, HashMap, Utc};
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
        // Recherche directe dans le registre des définitions
        let def = match self.definitions.get(workflow_id) {
            Some(definition) => definition,
            None => raise_error!(
                "ERR_WF_DEFINITION_NOT_FOUND",
                context = json!({
                    "workflow_id": workflow_id,
                    "action": "resolve_workflow_definition",
                    "hint": "La définition est absente du registre. Vérifiez le chargement des fichiers YAML/JSON au démarrage."
                })
            ),
        };

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
        // Recherche de la définition liée à l'instance active
        let def = match self.definitions.get(&instance.workflow_id) {
            Some(d) => d,
            None => raise_error!(
                "ERR_WF_INSTANCE_ORPHAN",
                context = json!({
                    "instance_id": instance.id,
                    "workflow_id": instance.workflow_id,
                    "action": "lookup_active_definition",
                    "hint": "Désynchronisation détectée : l'instance existe mais sa définition est absente du registre local."
                })
            ),
        };
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
                // Tentative de transition d'état dans le workflow
                if let Err(e) = sm.transition(instance, &node_id, status) {
                    raise_error!(
                        "ERR_WF_STATE_TRANSITION_FAILED",
                        context = json!({
                            "instance_id": instance.id,
                            "node_id": node_id,
                            "target_status": status,
                            "current_status": instance.status, // Changé de .state à .status
                            "error_details": e.to_string(),
                            "hint": "La transition a échoué. Vérifiez si l'état actuel permet d'atteindre le statut cible via ce nœud."
                        })
                    );
                }

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
        // 1. Appel asynchrone à la base
        let load_result = manager
            .get_document("workflow_instances", instance_id)
            .await;

        // 2. Résolution impérative et typée
        let doc = match load_result {
            Ok(Some(document)) => document,
            Ok(None) => raise_error!(
                "ERR_WF_INSTANCE_NOT_FOUND",
                context = json!({
                    "instance_id": instance_id,
                    "action": "resolve_instance_id",
                    "hint": "L'ID ne correspond à aucune instance active dans la collection 'workflow_instances'."
                })
            ),
            Err(e) => raise_error!(
                "ERR_WF_INSTANCE_DB_ACCESS",
                context = json!({
                    "instance_id": instance_id,
                    "db_error": e.to_string(),
                    "action": "load_instance_from_db",
                    "hint": "Échec technique lors de la lecture du document d'instance."
                })
            ),
        };

        // Désérialisation précise de l'instance de workflow
        let mut instance: WorkflowInstance = match serde_json::from_value(doc) {
            Ok(inst) => inst,
            Err(e) => raise_error!(
                "ERR_WF_INSTANCE_DESERIALIZATION",
                context = json!({
                    "instance_id": instance_id, // L'ID utilisé pour le fetch juste avant
                    "error_details": e.to_string(),
                    "line": e.line(),
                    "column": e.column(),
                    "action": "hydrate_instance_from_db",
                    "hint": "Le JSON stocké en base ne correspond plus à la structure WorkflowInstance. Vérifiez si une mise à jour du code a modifié les champs requis (status, node_states, etc.)."
                })
            ),
        };

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
        // 1. Appel asynchrone à la base
        let load_result = manager
            .get_document("workflow_instances", instance_id)
            .await;

        // 2. Résolution impérative et typée
        let doc = match load_result {
            Ok(Some(document)) => document,
            Ok(None) => raise_error!(
                "ERR_WF_INSTANCE_NOT_FOUND",
                context = json!({
                    "instance_id": instance_id,
                    "action": "resolve_instance_id",
                    "hint": "L'ID ne correspond à aucune instance active dans la collection 'workflow_instances'."
                })
            ),
            Err(e) => raise_error!(
                "ERR_WF_INSTANCE_DB_ACCESS",
                context = json!({
                    "instance_id": instance_id,
                    "db_error": e.to_string(),
                    "action": "load_instance_from_db",
                    "hint": "Échec technique lors de la lecture du document d'instance."
                })
            ),
        };

        // Désérialisation précise de l'instance de workflow
        let mut instance: WorkflowInstance = match serde_json::from_value(doc) {
            Ok(inst) => inst,
            Err(e) => raise_error!(
                "ERR_WF_INSTANCE_DESERIALIZATION",
                context = json!({
                    "instance_id": instance_id,
                    "error_details": e.to_string(),
                    "line": e.line(),
                    "column": e.column(),
                    "action": "hydrate_instance_from_db",
                    "hint": "Le JSON stocké en base ne correspond plus à la structure WorkflowInstance. Vérifiez les champs requis (status, node_states, context)."
                })
            ),
        };

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
        instance.updated_at = Utc::now().timestamp();
        // Transformation de l'instance en valeur JSON pour le stockage
        let json_val = match serde_json::to_value(&instance) {
            Ok(v) => v,
            Err(e) => raise_error!(
                "ERR_WF_INSTANCE_SERIALIZATION",
                context = json!({
                    "instance_id": instance.id,
                    "error_details": e.to_string(),
                    "action": "serialize_instance_for_db",
                    "hint": "L'objet instance contient des données incompatibles avec le format JSON (vérifiez les types dans le 'context')."
                })
            ),
        };
        // Tentative d'insertion en base de données
        if let Err(e) = manager.insert_raw("workflow_instances", &json_val).await {
            raise_error!(
                "ERR_WF_INSTANCE_PERSIST_FAIL",
                context = json!({
                    "collection": "workflow_instances",
                    "action": "insert_workflow_instance",
                    "db_error": e.to_string(),
                    // On extrait l'ID de manière sécurisée pour le contexte
                    "instance_id": json_val.get("id").and_then(|v| v.as_str()).unwrap_or("unknown"),
                    "hint": "L'écriture sur le disque a échoué. Vérifiez l'espace disque disponible, les permissions du dossier 'workflow_instances' ou l'intégrité de l'index."
                })
            );
        }

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
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::utils::{Arc, AsyncMutex};
    use crate::workflow_engine::{NodeType, WorkflowEdge, WorkflowNode};

    // 🎯 IMPORT DES OUTILS DE TEST MODERNES
    use crate::utils::data::json;
    use crate::utils::mock::{inject_mock_component, AgentDbSandbox};

    /// Prépare un environnement complet pour les tests du Scheduler (Orchestrator, Executor)
    async fn setup_test_environment(
        storage: Arc<crate::json_db::storage::StorageEngine>,
        config: &crate::utils::config::AppConfig,
    ) -> WorkflowScheduler {
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

        manager
            .create_collection(
                "workflow_instances",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        // 1. 🎯 INJECTION DES MOCKS : Pour que l'Orchestrateur IA démarre sans paniquer
        inject_mock_component(
            &manager,
            "llm",
            json!({ "provider": "mock", "model": "test" }),
        )
        .await;
        inject_mock_component(&manager, "rag", json!({ "provider": "mock" })).await;

        // 2. Création du Moteur
        let orch = AiOrchestrator::new(ProjectModel::default(), Some(storage.clone()))
            .await
            .unwrap();

        let pm = Arc::new(PluginManager::new(&storage, None));
        let executor = WorkflowExecutor::new(Arc::new(AsyncMutex::new(orch)), pm);

        WorkflowScheduler::new(executor)
    }

    /// Helper pour créer manuellement un DAG personnalisé pour les tests
    fn create_mock_workflow(
        id: &str,
        nodes: Vec<WorkflowNode>,
        edges: Vec<WorkflowEdge>,
    ) -> WorkflowDefinition {
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
        let sandbox = AgentDbSandbox::new().await;
        let mut scheduler = setup_test_environment(sandbox.db.clone(), &sandbox.config).await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

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
        let sandbox = AgentDbSandbox::new().await;
        let scheduler = setup_test_environment(sandbox.db.clone(), &sandbox.config).await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let result = scheduler.create_instance("wf_ghost", &manager).await;

        assert!(
            result.is_err(),
            "La création doit échouer si le workflow n'est pas chargé"
        );

        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("ERR_WF_DEFINITION_NOT_FOUND"),
            "Le code d'erreur devrait être ERR_WF_DEFINITION_NOT_FOUND."
        );

        let crate::utils::error::AppError::Structured(data) = err;
        assert_eq!(data.code, "ERR_WF_DEFINITION_NOT_FOUND");
        assert_eq!(data.context["workflow_id"], "wf_ghost");
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_scheduler_step_by_step_execution() {
        let sandbox = AgentDbSandbox::new().await;
        let mut scheduler = setup_test_environment(sandbox.db.clone(), &sandbox.config).await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

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
        let sandbox = AgentDbSandbox::new().await;
        let mut scheduler = setup_test_environment(sandbox.db.clone(), &sandbox.config).await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

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
        let sandbox = AgentDbSandbox::new().await;
        let mut scheduler = setup_test_environment(sandbox.db.clone(), &sandbox.config).await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

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
