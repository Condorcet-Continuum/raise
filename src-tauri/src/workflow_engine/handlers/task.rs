// FICHIER : src-tauri/src/workflow_engine/handlers/task.rs
use super::{HandlerContext, NodeHandler};
use crate::ai::assurance::xai::{ExplanationScope, XaiFrame, XaiMethod};
use crate::code_generator::graph_weaver::OntologyWeaver;
use crate::code_generator::toolchains::rust::RustToolchain;
use crate::utils::prelude::*;
use crate::workflow_engine::squad::{Squad, SquadStatus};
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};

pub struct TaskHandler;

#[async_interface]
impl NodeHandler for TaskHandler {
    fn node_type(&self) -> NodeType {
        NodeType::Task
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut UnorderedMap<String, JsonValue>,
        shared_ctx: &HandlerContext<'_>,
    ) -> RaiseResult<ExecutionStatus> {
        // ====================================================================
        // 1. IDENTIFICATION DE LA MISSION ET DE LA SQUAD
        // ====================================================================
        let mission_handle = match context.get("mission_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => raise_error!("ERR_MISSION_ID_MISSING_IN_CONTEXT"),
        };

        let mission_doc = match shared_ctx
            .manager
            .get_document("missions", mission_handle)
            .await?
        {
            Some(doc) => doc,
            None => raise_error!("ERR_MISSION_NOT_FOUND"),
        };
        let squad_handle = mission_doc["squad_id"].as_str().unwrap_or_default();

        user_info!(
            "INF_SQUAD_ASSIGNED",
            json_value!({"squad_id": squad_handle, "task_id": node.id})
        );

        let squad = Squad::fetch_from_store(shared_ctx.manager, squad_handle).await?;

        if squad.status != SquadStatus::Active {
            user_error!(
                "ERR_SQUAD_NOT_ACTIVE",
                json_value!({"squad_id": squad.handle, "status": format!("{:?}", squad.status)})
            );
            return Ok(ExecutionStatus::Failed);
        }

        let lead_agent_id = squad.lead_agent_id.to_string();

        // ====================================================================
        // 2. FORGEAGE DE L'INTENTION MACRO POUR L'ORCHESTRATEUR
        // ====================================================================
        let rich_mission = format!(
            "OBJECTIF DE PHASE : {}\n\nINSTRUCTIONS SPÉCIFIQUES : {:?}\n\nCONTEXTE JUMEAU NUMÉRIQUE : {:?}\n\nSQUAD LEAD : {}",
            node.name, node.params, context, lead_agent_id
        );

        // ====================================================================
        // 3. EXÉCUTION DE LA SQUAD (BOUCLE ACL)
        // ====================================================================
        let mut orch = shared_ctx.orchestrator.lock().await;
        let agent_result = orch.execute_workflow(&rich_mission).await?;

        let mut new_artifacts = context
            .get("generated_artifacts")
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();

        for artifact in &agent_result.artifacts {
            new_artifacts
                .push(crate::utils::data::json::serialize_to_value(artifact).unwrap_or_default());

            if artifact.id.starts_with("code_") || artifact.id.starts_with("module_") {
                let target_path =
                    PathBuf::from("src/generated").join(format!("{}.rs", artifact.id));

                user_info!(
                    "INF_CODEGEN_START",
                    json_value!({"element_id": artifact.id})
                );

                match OntologyWeaver::generate_and_validate(
                    shared_ctx.manager,
                    &artifact.id,
                    target_path,
                    &RustToolchain,
                )
                .await
                {
                    Ok(path) => {
                        user_success!(
                            "SUC_CODEGEN_READY",
                            json_value!({"path": path.to_string_lossy()})
                        );
                    }
                    Err(AppError::Structured(err_box)) => {
                        if err_box.code == "ERR_CODEGEN_TOOLCHAIN_REJECTED" {
                            let feedback = err_box
                                .context
                                .get("xai_feedback")
                                .cloned()
                                .unwrap_or(json_value!("Erreur de compilation inconnue"));

                            user_warn!(
                                "WRN_CODEGEN_REJECTED",
                                json_value!({
                                    "element_id": artifact.id,
                                    "feedback": feedback
                                })
                            );

                            return Ok(ExecutionStatus::Failed);
                        }

                        return Err(AppError::Structured(err_box));
                    }
                }
            }
        }
        context.insert(
            "generated_artifacts".to_string(),
            json_value!(new_artifacts),
        );

        // ====================================================================
        // 4. TRAÇABILITÉ (XAI) & AUDITABILITÉ
        // ====================================================================
        let mut xai = XaiFrame::new(
            &node.id,
            XaiMethod::ChainOfThought,
            ExplanationScope::Global,
        );
        xai.predicted_output = agent_result.message.clone();
        xai.input_snapshot = rich_mission;

        use crate::rules_engine::ast::Expr;
        let default_rules = vec![Expr::Contains {
            list: Box::new(Expr::Var("predicted_output".to_string())),
            value: Box::new(Expr::Val(json_value!("JSON"))),
        }];

        let critique = match shared_ctx
            .critic
            .evaluate(&xai, shared_ctx.manager, &default_rules)
            .await
        {
            Ok(c) => c,
            Err(e) => raise_error!(
                "ERR_CRITIC_EXECUTION_FAILED",
                error = e,
                context = json_value!({"node_id": node.id})
            ),
        };
        if !critique.is_acceptable {
            user_warn!(
                "WRN_CRITIC_REJECTION",
                json_value!({
                    "reasoning": critique.reasoning,
                    "score": critique.score,
                    "node_id": node.id
                })
            );
        }

        // ====================================================================
        // 5. PERSISTANCE
        // ====================================================================
        let xai_id = format!(
            "ref:xai_frames:handle:xai_{}_{}",
            node.id,
            UtcClock::now().timestamp_millis()
        );
        let mut xai_json = json::serialize_to_value(&xai).unwrap_or(json_value!({}));

        if let Some(obj) = xai_json.as_object_mut() {
            obj.insert("_id".to_string(), json_value!(xai_id.clone()));
            obj.insert("fidelity_score".to_string(), json_value!(critique.score));
        }

        let _ = shared_ctx
            .manager
            .create_collection(
                "xai_frames",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await;
        let _ = shared_ctx
            .manager
            .upsert_document("xai_frames", xai_json)
            .await;

        let mut traces = context
            .get("xai_traces")
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        traces.push(json_value!(xai_id));
        context.insert("xai_traces".to_string(), json_value!(traces));

        let output_key = node
            .params
            .get("output_key")
            .and_then(|v| v.as_str())
            .unwrap_or("task_output");
        context.insert(output_key.to_string(), json_value!(agent_result.message));

        user_success!(
            "SUC_TASK_COMPLETED",
            json_value!({"task_name": node.name, "node_id": node.id})
        );
        Ok(ExecutionStatus::Completed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::orchestrator::AiOrchestrator;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};
    use crate::workflow_engine::critic::WorkflowCritic;

    async fn setup_dummy_context<'a>(
        storage: SharedRef<crate::json_db::storage::StorageEngine>,
        config: &'a AppConfig,
        sandbox_db: &'a crate::json_db::storage::StorageEngine,
    ) -> (
        SharedRef<AsyncMutex<AiOrchestrator>>,
        SharedRef<PluginManager>,
        WorkflowCritic,
        UnorderedMap<String, Box<dyn crate::workflow_engine::tools::AgentTool>>,
        CollectionsManager<'a>,
    ) {
        let manager = CollectionsManager::new(sandbox_db, &config.system_domain, &config.system_db);

        inject_mock_component(
            &manager,
            "llm",
            json_value!({ "provider": "mock", "model": "test" }),
        )
        .await;
        inject_mock_component(&manager, "rag", json_value!({ "provider": "mock" })).await;

        let orch = AiOrchestrator::new(ProjectModel::default(), &manager, storage.clone())
            .await
            .unwrap();
        (
            SharedRef::new(AsyncMutex::new(orch)),
            SharedRef::new(PluginManager::new(&storage, None)),
            WorkflowCritic::default(),
            UnorderedMap::new(),
            manager,
        )
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_task_handler_squad_delegation() {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools, manager) =
            setup_dummy_context(sandbox.db.clone(), &sandbox.config, &sandbox.db).await;

        let generic_schema = "db://_system/_system/schemas/v1/db/generic.schema.json";

        // 🎯 L'UUID strict pour valider le chargement de la Squad
        let lead_agent_uuid = "10000000-0000-0000-0000-000000000001";

        let mock_agent = |id: &str| {
            let handle = id.replace("-", "").replace("ref:agents:handle:", ""); // Nettoyage pour le slug
            json_value!({
                "_id": id,
                "handle": handle,
                "name": handle,
                "status": "active",
                "description": "Mock Agent",
                "neuroProfile": { "promptId": "ref:prompts:handle:dummy" },
                "base": { "neuro_profile": { "prompt_id": "ref:prompts:handle:dummy" } }
            })
        };

        let mock_prompt = json_value!({
            "_id": "ref:prompts:handle:dummy",
            "handle": "dummy",
            "name": "Dummy Prompt",
            "role": "Assistant de Test",
            "identity": { "persona": "Tu es un assistant de test.", "tone": "professionnel" },
            "environment": "Environnement de test simulé pour la Squad.",
            "directives": ["Exécute la tâche de test", "Génère un JSON valide"]
        });

        let o = orch.lock().await;
        let project_manager = CollectionsManager::new(&sandbox.db, &o.space, &o.db_name);

        let collections = vec!["prompts", "agents", "configs", "session_agents"];
        for coll in collections {
            let _ = manager.create_collection(coll, generic_schema).await;
            let _ = project_manager
                .create_collection(coll, generic_schema)
                .await;

            if coll == "prompts" {
                let _ = manager.upsert_document(coll, mock_prompt.clone()).await;
                let _ = project_manager
                    .upsert_document(coll, mock_prompt.clone())
                    .await;
            } else if coll == "agents" {
                // 1. On insère l'Agent UUID pour satisfaire la Squad
                let _ = manager
                    .upsert_document(coll, mock_agent(lead_agent_uuid))
                    .await;

                // 2. On insère les URI stricts pour satisfaire l'Orchestrateur Mock !
                let _ = manager
                    .upsert_document(coll, mock_agent("ref:agents:handle:agent_software"))
                    .await;
                let _ = manager
                    .upsert_document(coll, mock_agent("ref:agents:handle:agent_lead_architect"))
                    .await;
                let _ = manager
                    .upsert_document(coll, mock_agent("ref:agents:handle:agent_quality"))
                    .await;

                // (Même chose côté Projet)
                let _ = project_manager
                    .upsert_document(coll, mock_agent(lead_agent_uuid))
                    .await;
                let _ = project_manager
                    .upsert_document(coll, mock_agent("ref:agents:handle:agent_software"))
                    .await;
                let _ = project_manager
                    .upsert_document(coll, mock_agent("ref:agents:handle:agent_lead_architect"))
                    .await;
            }
        }
        drop(o);

        // 1. Mocker la Squad (avec l'UUID de l'agent en lead)
        let _ = manager.create_collection("squads", generic_schema).await;
        manager
            .upsert_document(
                "squads",
                json_value!({
                    "_id": "squad_01",
                    "handle": "squad-01",
                    "name": "Squad Architecture",
                    "lead_agent_id": lead_agent_uuid,
                    "status": "active"
                }),
            )
            .await
            .unwrap();

        // 2. Mocker la Mission
        let _ = manager.create_collection("missions", generic_schema).await;
        manager
            .upsert_document(
                "missions",
                json_value!({
                    "_id": "mission_123",
                    "handle": "mission-123",
                    "name": "Conception Freinage",
                    "squad_id": "squad-01",
                    "mandate_id": "man_1",
                    "status": "running"
                }),
            )
            .await
            .unwrap();

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager,
        };

        let node = WorkflowNode {
            id: "task_phase_la".into(),
            r#type: NodeType::Task,
            name: "Phase d'Architecture Logique".into(),
            params: json_value!({ "output_key": "la_report" }),
        };

        let mut data_ctx = UnorderedMap::new();
        data_ctx.insert("mission_id".to_string(), json_value!("mission-123"));

        let result = TaskHandler
            .execute(&node, &mut data_ctx, &ctx)
            .await
            .unwrap();

        assert_eq!(result, ExecutionStatus::Completed);
    }
}
