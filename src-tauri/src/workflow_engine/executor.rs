// FICHIER : src-tauri/src/workflow_engine/executor.rs

use super::compiler::WorkflowCompiler;
use super::mandate::Mandate;
use super::tools::AgentTool;
// REMPLACEMENT : On utilise le PluginManager du Hub au lieu du WasmHost isolé
use super::{critic::WorkflowCritic, ExecutionStatus, NodeType, WorkflowDefinition, WorkflowNode};
use crate::plugins::manager::PluginManager;

use crate::ai::assurance::xai::{ExplanationScope, XaiFrame, XaiMethod};
use crate::ai::orchestrator::AiOrchestrator;
use crate::json_db::collections::manager::CollectionsManager;
// AJOUT : Intégration du moteur de règles
use crate::rules_engine::ast::Expr;
use crate::rules_engine::evaluator::{Evaluator, NoOpDataProvider};
use crate::utils::{AppError, Result};

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// L'Exécuteur transforme les intentions du workflow en actions via l'IA ou des Outils.
pub struct WorkflowExecutor {
    /// Référence partagée vers l'Orchestrateur IA
    pub orchestrator: Arc<Mutex<AiOrchestrator>>,
    /// Gestionnaire de plugins pour l'exécution WASM cognitive
    pub plugin_manager: Arc<PluginManager>,
    /// Module de Critique (Reward Model)
    critic: WorkflowCritic,
    /// Registre des outils disponibles (MCP)
    tools: HashMap<String, Box<dyn AgentTool>>,
}

impl WorkflowExecutor {
    /// Crée un nouvel exécuteur lié à l'intelligence centrale et au Hub de Plugins
    pub fn new(
        orchestrator: Arc<Mutex<AiOrchestrator>>,
        plugin_manager: Arc<PluginManager>,
    ) -> Self {
        Self {
            orchestrator,
            plugin_manager,
            critic: WorkflowCritic::default(),
            tools: HashMap::new(), // Initialisation vide
        }
    }

    /// Permet au Scheduler d'injecter des outils
    pub fn register_tool(&mut self, tool: Box<dyn AgentTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    // ========================================================================
    // LE PONT : Chargement et Compilation Sécurisés
    // ========================================================================

    /// Point d'entrée sécurisé pour charger un mandat et préparer l'exécution.
    pub async fn load_and_prepare_workflow(
        manager: &CollectionsManager<'_>,
        mandate_id: &str,
    ) -> Result<WorkflowDefinition> {
        // 1. PONT : Chargement validé (Fail-Fast si le JSON est invalide)
        let mandate = Mandate::fetch_from_store(manager, mandate_id).await?;

        tracing::info!(
            "📜 Mandat chargé et validé : {} v{} (Stratégie: {:?})",
            mandate.meta.author,
            mandate.meta.version,
            mandate.governance.strategy
        );

        // 2. COMPILATION : Transformation en graphe technique
        let workflow = WorkflowCompiler::compile(&mandate);

        tracing::info!(
            "🏗️ Workflow compilé avec succès : {} ({}) - {} noeuds",
            workflow.id,
            mandate.id,
            workflow.nodes.len()
        );

        Ok(workflow)
    }

    // ========================================================================
    // EXECUTION DES NOEUDS
    // ========================================================================

    /// Point d'entrée pour l'exécution d'un nœud du graphe
    pub async fn execute_node(
        &self,
        node: &WorkflowNode,
        context: &mut HashMap<String, Value>,
    ) -> Result<ExecutionStatus> {
        tracing::info!("⚙️ Exécution Agentique : {} ({:?})", node.name, node.r#type);

        match node.r#type {
            // Tâche standard (IA + Critique)
            NodeType::Task => self.handle_agentic_task(node, context).await,

            // Décision Algorithmique (Condorcet Pondéré)
            NodeType::Decision => self.handle_decision(node, context).await,

            // Gestion des Lignes Rouges (Vetos du Mandat)
            NodeType::GatePolicy => self.handle_policy_gate(node, context).await,

            // Appel d'outil MCP
            NodeType::CallMcp => self.handle_tool_call(node, context).await,

            // ================================================================
            // INTÉGRATION HUB : Exécution de Module WASM (Hot-Swap & Cognitif)
            // ================================================================
            NodeType::Wasm => {
                let plugin_id = node
                    .params
                    .get("plugin_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&node.id);

                tracing::info!("🔮 [WASM Hub] Appel du plugin : {}", plugin_id);

                // Récupération du Mandat depuis le contexte pour injection
                let mandate_ctx = context.get("_mandate").cloned();

                let start = std::time::Instant::now();

                // Exécution via le PluginManager (accès DB, AI, Rules inclus)
                match self
                    .plugin_manager
                    .run_plugin_with_context(plugin_id, mandate_ctx)
                {
                    Ok((exit_code, signals)) => {
                        let duration = start.elapsed();
                        tracing::info!(
                            "🔮 [WASM] Exécution terminée en {:?} (Code: {})",
                            duration,
                            exit_code
                        );

                        // Traitement des Signaux (Feedback vers le Workflow)
                        for signal in signals {
                            tracing::info!("📡 [SIGNAL PLUGIN] {} : {:?}", plugin_id, signal);
                            context.insert(format!("{}_signal", plugin_id), signal);
                        }

                        if exit_code == 1 {
                            Ok(ExecutionStatus::Completed)
                        } else {
                            tracing::warn!(
                                "⛔ [WASM VETO] Plugin a retourné un échec (Code {})",
                                exit_code
                            );
                            Ok(ExecutionStatus::Failed)
                        }
                    }
                    Err(e) => {
                        tracing::error!("❌ [WASM ERROR] Échec exécution : {}", e);
                        Ok(ExecutionStatus::Failed)
                    }
                }
            }

            NodeType::GateHitl => {
                tracing::warn!("⏸️ Workflow en pause : '{}'", node.name);
                Ok(ExecutionStatus::Paused)
            }

            NodeType::End => Ok(ExecutionStatus::Completed),

            _ => Ok(ExecutionStatus::Completed),
        }
    }

    async fn handle_tool_call(
        &self,
        node: &WorkflowNode,
        context: &mut HashMap<String, Value>,
    ) -> Result<ExecutionStatus> {
        let tool_name = node
            .params
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::from("Paramètre 'tool_name' manquant pour CallMcp".to_string())
            })?;

        let default_args = json!({});
        let args = node.params.get("arguments").unwrap_or(&default_args);

        tracing::info!("🛠️ Appel Outil MCP : {} avec {:?}", tool_name, args);

        if let Some(tool) = self.tools.get(tool_name) {
            match tool.execute(args).await {
                Ok(output) => {
                    tracing::info!("✅ Résultat Outil : {:?}", output);

                    if tool_name == "read_system_metrics" {
                        context.insert("sensor_vibration".to_string(), output);
                    }

                    Ok(ExecutionStatus::Completed)
                }
                Err(e) => {
                    tracing::error!("❌ Erreur outil : {}", e);
                    Ok(ExecutionStatus::Failed)
                }
            }
        } else {
            tracing::error!("❌ Outil introuvable : {}", tool_name);
            Ok(ExecutionStatus::Failed)
        }
    }

    async fn handle_policy_gate(
        &self,
        node: &WorkflowNode,
        context: &HashMap<String, Value>,
    ) -> Result<ExecutionStatus> {
        let rule_name = node
            .params
            .get("rule")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN");
        tracing::info!("🛡️ Vérification Veto : {}", rule_name);

        // 1. MODE DYNAMIQUE (via Rules Engine)
        if let Some(ast_val) = node.params.get("ast") {
            if let Ok(expr) = serde_json::from_value::<Expr>(ast_val.clone()) {
                // Conversion du contexte HashMap -> Value
                let context_value = serde_json::to_value(context).unwrap_or(json!({}));
                let provider = NoOpDataProvider; // Pas de lookup DB pour l'instant dans ce scope

                match Evaluator::evaluate(&expr, &context_value, &provider).await {
                    Ok(res_cow) => {
                        let res = res_cow.as_ref();
                        // Convention Veto : Si la règle renvoie TRUE, on BLOQUE (Fail)
                        let is_triggered = match res {
                            Value::Bool(b) => *b,
                            _ => false, // Par défaut, si pas booléen, on ne bloque pas
                        };

                        if is_triggered {
                            tracing::error!("🚨 VETO DYNAMIQUE DÉCLENCHÉ : {}", rule_name);
                            return Ok(ExecutionStatus::Failed);
                        } else {
                            return Ok(ExecutionStatus::Completed);
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            "❌ Erreur lors de l'évaluation de la règle dynamique : {}",
                            e
                        );
                    }
                }
            }
        }

        // 2. MODE LEGACY (Hardcodé pour rétrocompatibilité)
        if rule_name == "VIBRATION_MAX" {
            let current_vibration =
                if let Some(obj) = context.get("sensor_vibration").and_then(|v| v.as_object()) {
                    obj.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0)
                } else {
                    context
                        .get("sensor_vibration")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0)
                };

            let threshold = 8.0;

            if current_vibration > threshold {
                tracing::error!(
                    "🚨 VETO DÉCLENCHÉ : Vibration {:.2} > Seuil {:.2}",
                    current_vibration,
                    threshold
                );
                return Ok(ExecutionStatus::Failed);
            }
        }

        Ok(ExecutionStatus::Completed)
    }

    async fn handle_decision(
        &self,
        node: &WorkflowNode,
        context: &HashMap<String, Value>,
    ) -> Result<ExecutionStatus> {
        tracing::info!("🗳️ Algorithme de Condorcet : {}", node.name);

        let default_weights = serde_json::Map::new();
        let weights = node
            .params
            .get("weights")
            .and_then(|v| v.as_object())
            .unwrap_or(&default_weights);

        let w_security = weights
            .get("agent_security")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        let w_finance = weights
            .get("agent_finance")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);

        let candidates = match context.get("candidates").and_then(|v| v.as_array()) {
            Some(list) if list.len() > 1 => list,
            _ => return Ok(ExecutionStatus::Completed),
        };

        let mut wins = vec![0.0; candidates.len()];

        for i in 0..candidates.len() {
            for j in (i + 1)..candidates.len() {
                let cand_a = &candidates[i];
                let cand_b = &candidates[j];
                let len_a = cand_a.to_string().len();
                let len_b = cand_b.to_string().len();

                if len_a < len_b {
                    wins[i] += w_security;
                } else {
                    wins[j] += w_security;
                }
                if len_a > len_b {
                    wins[i] += w_finance;
                } else {
                    wins[j] += w_finance;
                }
            }
        }

        let (winner_idx, max_wins) = wins
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();

        tracing::info!(
            "👑 Vainqueur Condorcet : Candidat #{} (Score: {:.1})",
            winner_idx,
            max_wins
        );
        Ok(ExecutionStatus::Completed)
    }

    async fn handle_agentic_task(
        &self,
        node: &WorkflowNode,
        context: &HashMap<String, Value>,
    ) -> Result<ExecutionStatus> {
        let mut orch = self.orchestrator.lock().await;

        let mission = format!(
            "OBJECTIF: {}\nPARAMÈTRES: {:?}\nCONTEXTE: {:?}",
            node.name, node.params, context
        );

        let ai_response = orch.ask(&mission).await?;

        let mut xai = XaiFrame::new(&node.id, XaiMethod::ChainOfThought, ExplanationScope::Local);
        xai.predicted_output = ai_response.clone();
        xai.input_snapshot = mission;

        let critique = self.critic.evaluate(&xai).await;
        if !critique.is_acceptable {
            tracing::warn!("⚠️ Qualité insuffisante détectée par le critique !");
        }

        tracing::info!("✅ Tâche '{}' validée par l'agent.", node.name);
        Ok(ExecutionStatus::Completed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::model_engine::types::ProjectModel;
    use crate::workflow_engine::tools::SystemMonitorTool;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::Mutex;

    use crate::json_db::schema::registry::SchemaRegistry;
    use crate::json_db::schema::validator::SchemaValidator;
    use crate::json_db::test_utils::{ensure_db_exists, init_test_env};

    async fn create_test_executor_with_tools() -> WorkflowExecutor {
        let model = ProjectModel::default();
        // CORRECTION E0061 : Ajout de None pour l'argument StorageEngine (pas nécessaire dans ce mock)
        let orch = AiOrchestrator::new(
            model,
            "http://127.0.0.1:6334",
            "http://127.0.0.1:8081",
            None,
        )
        .await
        .unwrap_or_else(|_| panic!("Mock fail"));

        // Initialisation de la dépendance PluginManager pour les tests
        let dir = tempdir().unwrap();
        let storage = StorageEngine::new(JsonDbConfig::new(dir.path().to_path_buf()));
        let plugin_manager = Arc::new(PluginManager::new(&storage, None));

        let mut exec = WorkflowExecutor::new(Arc::new(Mutex::new(orch)), plugin_manager);
        exec.register_tool(Box::new(SystemMonitorTool));
        exec
    }

    fn to_ctx(val: Value) -> HashMap<String, Value> {
        serde_json::from_value(val).unwrap_or_default()
    }

    #[tokio::test]
    #[ignore = "Nécessite connexion orchestrateur"]
    async fn test_gate_pause() {
        let executor = create_test_executor_with_tools().await;
        let node = WorkflowNode {
            id: "node_pause".into(),
            r#type: NodeType::GateHitl,
            name: "Human Check".into(),
            params: Value::Null,
        };
        let mut ctx = HashMap::new();
        let result = executor.execute_node(&node, &mut ctx).await;
        assert_eq!(result.unwrap(), ExecutionStatus::Paused);
    }

    #[tokio::test]
    #[ignore = "Nécessite connexion orchestrateur"]
    async fn test_policy_veto_trigger() {
        let executor = create_test_executor_with_tools().await;

        let node = WorkflowNode {
            id: "gate_veto".into(),
            r#type: NodeType::GatePolicy,
            name: "VETO: VIBRATION_MAX".into(),
            params: json!({ "rule": "VIBRATION_MAX" }),
        };

        let mut ctx_ok = to_ctx(json!({ "sensor_vibration": 2.5 }));
        let res_ok = executor.execute_node(&node, &mut ctx_ok).await;
        assert_eq!(res_ok.unwrap(), ExecutionStatus::Completed);

        let mut ctx_danger = to_ctx(json!({ "sensor_vibration": 12.0 }));
        let res_danger = executor.execute_node(&node, &mut ctx_danger).await;
        assert_eq!(res_danger.unwrap(), ExecutionStatus::Failed);
    }

    #[tokio::test]
    #[ignore = "Nécessite connexion orchestrateur"]
    async fn test_policy_veto_dynamic() {
        let executor = create_test_executor_with_tools().await;

        // Règle AST : Trigger veto si temperature > 100
        let ast_veto = json!({
            "Gt": [
                { "Var": "temperature" },
                { "Val": 100.0 }
            ]
        });

        let node = WorkflowNode {
            id: "gate_dynamic".into(),
            r#type: NodeType::GatePolicy,
            name: "VETO: HIGH_TEMP".into(),
            params: json!({
                "rule": "HIGH_TEMP",
                "ast": ast_veto
            }),
        };

        // Cas OK (Temp = 50, règle false, pas de veto)
        let mut ctx_ok = to_ctx(json!({ "temperature": 50.0 }));
        let res_ok = executor.execute_node(&node, &mut ctx_ok).await;
        assert_eq!(res_ok.unwrap(), ExecutionStatus::Completed);

        // Cas KO (Temp = 150, règle true, veto activé)
        let mut ctx_fail = to_ctx(json!({ "temperature": 150.0 }));
        let res_fail = executor.execute_node(&node, &mut ctx_fail).await;
        assert_eq!(res_fail.unwrap(), ExecutionStatus::Failed);
    }

    #[tokio::test]
    #[ignore = "Nécessite connexion orchestrateur"]
    async fn test_weighted_condorcet() {
        let executor = create_test_executor_with_tools().await;

        let mut context = to_ctx(json!({
            "candidates": ["Short", "Very Long Option"]
        }));

        let node_finance = WorkflowNode {
            id: "vote".into(),
            r#type: NodeType::Decision,
            name: "Vote".into(),
            params: json!({ "weights": { "agent_security": 1.0, "agent_finance": 3.0 } }),
        };
        let res = executor.execute_node(&node_finance, &mut context).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_bridge_loading_and_compilation() {
        let env = init_test_env().await;
        let cfg = &env.cfg;
        let space = &env.space;
        let db = &env.db;

        ensure_db_exists(cfg, space, db).await;

        // --- ÉTAPE 1 : PRÉPARATION PHYSIQUE DES FICHIERS ---
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let src_schemas = manifest_dir.join("../schemas/v1");

        let dest_schemas = cfg.db_schemas_root(space, db).join("v1");
        std::fs::create_dir_all(&dest_schemas).unwrap();

        let dest_mandate_path = dest_schemas.join("mandates.json");
        let mut file_created = false;

        if src_schemas.exists() {
            if std::fs::copy(src_schemas.join("mandates.json"), &dest_mandate_path).is_ok() {
                file_created = true;
            }
        }

        if !file_created {
            let fallback = json!({
                "type": "object",
                "properties": { "id": { "type": "string" } },
                "required": ["id"]
            });
            std::fs::write(&dest_mandate_path, fallback.to_string()).unwrap();
        }

        // --- ÉTAPE 2 : INITIALISATION DU REGISTRE ---
        let reg = SchemaRegistry::from_db(cfg, space, db).expect("registry init failed");
        let root_uri = reg.uri("mandates.json");

        // --- ÉTAPE 3 : COMPILATION ---
        let _validator = SchemaValidator::compile_with_registry(&root_uri, &reg)
            .expect("Échec compilation schéma mandates pour le Pont");

        // --- ÉTAPE 4 : EXÉCUTION DU TEST ---
        let manager = CollectionsManager::new(&env.storage, space, db);

        manager
            .create_collection("mandates", Some("mandates.json".to_string()))
            .await
            .expect("Échec création collection mandates");
        let valid_mandate = json!({
            "id": "mandate_prod",
            "meta": { "author": "BridgeTest", "version": "1.0", "status": "ACTIVE" },
            "governance": {
                "strategy": "SAFETY_FIRST",
                "condorcetWeights": {
                    "agent_security": 1.0,
                    "agent_finance": 1.0
                }
            },
            "hardLogic": {
                "vetos": [
                    { "rule": "VIBRATION_MAX", "active": true, "action": "STOP" }
                ]
            },
            "observability": { "heartbeatMs": 100 }
        });

        manager
            .insert_raw("mandates", &valid_mandate)
            .await
            .unwrap();

        let result = WorkflowExecutor::load_and_prepare_workflow(&manager, "mandate_prod").await;

        assert!(
            result.is_ok(),
            "Le chargement et la compilation doivent réussir : {:?}",
            result.err()
        );

        let workflow = result.unwrap();
        assert!(workflow.nodes.len() >= 4);
        assert!(workflow
            .nodes
            .iter()
            .any(|n| n.name.contains("VIBRATION_MAX")));
    }
}
