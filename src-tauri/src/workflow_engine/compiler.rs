// FICHIER : src-tauri/src/workflow_engine/compiler.rs
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

use super::mandate::Mandate;
use super::{NodeType, WorkflowDefinition, WorkflowEdge, WorkflowNode};

pub struct WorkflowCompiler;

impl WorkflowCompiler {
    /// 🎯 DATA-DRIVEN : Résout les dépendances techniques depuis la base de données.
    async fn resolve_tool_dependency(
        manager: &CollectionsManager<'_>,
        rule_name: &str,
    ) -> Option<(String, JsonValue, String)> {
        if let Ok(Some(doc)) = manager
            .get_document("configs", "ref:configs:tool_dependencies")
            .await
        {
            if let Some(mapping) = doc.get("mappings").and_then(|m| m.as_object()) {
                if let Some(rule_config) = mapping.get(rule_name).and_then(|r| r.as_object()) {
                    let tool_name = rule_config
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let arguments = rule_config
                        .get("arguments")
                        .cloned()
                        .unwrap_or(json_value!({}));
                    let output_key = rule_config
                        .get("output_key")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();

                    if !tool_name.is_empty() {
                        return Some((tool_name, arguments, output_key));
                    }
                }
            }
        }
        None
    }

    /// 🎯 NOUVEAU : Compile dynamiquement un workflow à partir d'une Mission
    pub async fn compile(
        manager: &CollectionsManager<'_>,
        mission_handle: &str,
    ) -> RaiseResult<WorkflowDefinition> {
        // 1. Charger la Mission
        let mission_doc = match manager.get_document("missions", mission_handle).await? {
            Some(doc) => doc,
            None => raise_error!(
                "ERR_MISSION_NOT_FOUND",
                context = json_value!({"mission_id": mission_handle})
            ),
        };

        let template_handle = mission_doc["workflow_template_id"]
            .as_str()
            .unwrap_or_default();
        let mandate_handle = mission_doc["mandate_id"].as_str().unwrap_or_default();

        // 2. Charger le WorkflowTemplate (Graphe métier de base)
        let template_doc = match manager
            .get_document("workflow_definitions", template_handle)
            .await?
        {
            Some(doc) => doc,
            None => raise_error!(
                "ERR_TEMPLATE_NOT_FOUND",
                context = json_value!({"template_id": template_handle})
            ),
        };
        let mut workflow: WorkflowDefinition = json::deserialize_from_value(template_doc).unwrap();

        // 3. Charger le Mandat (Règles de gouvernance)
        let mandate = Mandate::fetch_from_store(manager, mandate_handle).await?;

        // 4. "Weaving" (Tissage) : Injection des vetos du Mandat dans le Workflow
        let original_entry = workflow.entry.clone();
        let mut previous_node_id = original_entry.clone();

        // On récupère les arêtes qui partaient de l'entrée pour les re-brancher à la fin de nos vetos
        let entry_edges: Vec<WorkflowEdge> = workflow
            .edges
            .iter()
            .filter(|e| e.from == original_entry)
            .cloned()
            .collect();
        workflow.edges.retain(|e| e.from != original_entry); // On enlève les arêtes d'origine

        for (i, veto) in mandate.hard_logic.vetos.iter().enumerate() {
            if veto.active {
                // Si la règle nécessite un capteur externe (CallMcp)
                if let Some((tool_name, args, output_key)) =
                    Self::resolve_tool_dependency(manager, &veto.rule).await
                {
                    let tool_node_id = format!("tool_read_{}_{}", veto.rule.to_lowercase(), i);
                    workflow.nodes.push(WorkflowNode {
                        id: tool_node_id.clone(),
                        r#type: NodeType::CallMcp,
                        name: format!("Lecture pour {}", veto.rule),
                        params: json_value!({
                            "tool_name": tool_name,
                            "arguments": args,
                            "output_key": output_key
                        }),
                    });
                    workflow.edges.push(WorkflowEdge {
                        from: previous_node_id.clone(),
                        to: tool_node_id.clone(),
                        condition: None,
                    });
                    previous_node_id = tool_node_id;
                }

                // Le Nœud QualityGate (Ex-GatePolicy)
                let node_id = format!("quality_gate_{}_{}", veto.rule.to_lowercase(), i);
                let mut params = json_value!({
                    "rule": veto.rule,
                    "action": veto.action
                });
                if let Some(ast) = &veto.ast {
                    params
                        .as_object_mut()
                        .unwrap()
                        .insert("ast".to_string(), ast.clone());
                }

                workflow.nodes.push(WorkflowNode {
                    id: node_id.clone(),
                    r#type: NodeType::QualityGate, // NOUVEAU NOM
                    name: format!("Vérification: {}", veto.rule),
                    params,
                });

                workflow.edges.push(WorkflowEdge {
                    from: previous_node_id.clone(),
                    to: node_id.clone(),
                    condition: None,
                });
                previous_node_id = node_id;
            }
        }

        // Re-brancher la fin des vetos injectés vers la suite du graphe original
        for mut edge in entry_edges {
            edge.from = previous_node_id.clone();
            workflow.edges.push(edge);
        }

        // L'ID final est unique à cette exécution
        workflow._id = None;
        workflow.handle = format!(
            "wf_compiled_{}_{}",
            mandate_handle,
            UtcClock::now().timestamp_millis()
        );
        Ok(workflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_compiler_mission_weaving() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // 1. Créer le template de Workflow (Un simple nœud start -> task -> end)
        manager
            .create_collection(
                "workflow_definitions",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .upsert_document(
                "workflow_definitions",
                json_value!({
                    "handle": "tpl_mbse_v1",
                    "entry": "start",
                    "nodes": [
                        { "id": "start", "type": "task", "name": "Start", "params": {} },
                        { "id": "task_1", "type": "task", "name": "Phase LA", "params": {} }
                    ],
                    "edges": [{ "from": "start", "to": "task_1", "condition": null }]
                }),
            )
            .await
            .unwrap();

        // 2. Créer le Mandat avec un veto
        manager
            .create_collection(
                "mandates",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager.upsert_document("mandates", json_value!({
            "handle": "man_123",
            "handle": "mandate-123",
            "name": "Mandat 123",
            "meta": { "author": "Admin", "version": "1.0", "status": "ACTIVE" },
            "governance": { "strategy": "SAFETY_FIRST", "condorcetWeights": {} },
            "hardLogic": {
                "vetos": [{ "rule": "ISO_26262_CHK", "active": true, "action": "STOP", "ast": {"Eq": [{"Var": "x"}, {"Val": 1}]} }]
            },
            "observability": { "heartbeatMs": 100 }
        })).await.unwrap();

        // 3. Créer la Mission qui lie les deux
        manager
            .create_collection(
                "missions",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .upsert_document(
                "missions",
                json_value!({
                    "handle": "mission_alpha",
                    "name": "Mission Alpha",
                    "mandate_id": "mandate-123",
                    "squad_id": "squad_arch",
                    "workflow_template_id": "tpl_mbse_v1",
                    "status": "draft"
                }),
            )
            .await
            .unwrap();

        let wf = WorkflowCompiler::compile(&manager, "mission_alpha")
            .await
            .unwrap();

        // Le graphe final doit avoir : start + quality_gate_iso_26262_chk_0 + task_1 (3 nœuds)
        assert_eq!(wf.nodes.len(), 3);

        let gates: Vec<_> = wf
            .nodes
            .iter()
            .filter(|n| n.r#type == NodeType::QualityGate)
            .collect();
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].id, "quality_gate_iso_26262_chk_0");

        // Vérifier le chaînage (Start -> Gate -> Task)
        assert!(wf
            .edges
            .iter()
            .any(|e| e.from == "start" && e.to == "quality_gate_iso_26262_chk_0"));
        assert!(wf
            .edges
            .iter()
            .any(|e| e.from == "quality_gate_iso_26262_chk_0" && e.to == "task_1"));
    }
}
