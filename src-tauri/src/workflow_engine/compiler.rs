// FICHIER : src-tauri/src/workflow_engine/compiler.rs
use crate::utils::prelude::*;

use super::mandate::Mandate;
use super::{NodeType, WorkflowDefinition, WorkflowEdge, WorkflowNode};

pub struct WorkflowCompiler;

impl WorkflowCompiler {
    /// Dictionnaire interne pour résoudre les dépendances techniques d'une règle politique
    /// Retourne : Option<(Nom_de_l_outil, Arguments_JSON, Clé_de_contexte_cible)>
    fn resolve_tool_dependency(rule_name: &str) -> Option<(&'static str, JsonValue, &'static str)> {
        match rule_name {
            "VIBRATION_MAX" => Some((
                "read_system_metrics",
                json_value!({ "sensor_id": "vibration_z" }),
                "sensor_vibration",
            )),
            "TEMP_MAX" => Some((
                "read_system_metrics",
                json_value!({ "sensor_id": "temp_core" }),
                "sensor_temperature",
            )),
            // Facilement extensible sans modifier le cœur du moteur
            _ => None,
        }
    }

    /// Transforme un Mandat politique en Workflow technique exécutable
    pub fn compile(mandate: &Mandate) -> WorkflowDefinition {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        let wf_id = format!(
            "wf_{}_{}",
            mandate.meta.author.replace(" ", ""),
            mandate.meta.version
        );

        let mut previous_node_id = "start".to_string();

        // 1. Nœud de Départ
        nodes.push(WorkflowNode {
            id: "start".into(),
            r#type: NodeType::Task,
            name: "Initialisation Mandat".into(),
            params: json_value!({
                "strategy": mandate.governance.strategy,
                "observability": mandate.observability
            }),
        });

        // 2. Compilation des Lignes Rouges (VETOS -> GatePolicy)
        for (i, veto) in mandate.hard_logic.vetos.iter().enumerate() {
            if veto.active {
                // Injection DYNAMIQUE de l'outil si la règle le nécessite
                if let Some((tool_name, args, output_key)) =
                    Self::resolve_tool_dependency(&veto.rule)
                {
                    let tool_node_id = format!("tool_read_{}_{}", veto.rule.to_lowercase(), i);

                    nodes.push(WorkflowNode {
                        id: tool_node_id.clone(),
                        r#type: NodeType::CallMcp,
                        name: format!("Lecture pour {}", veto.rule),
                        params: json_value!({
                            "tool_name": tool_name,
                            "arguments": args,
                            "output_key": output_key // Instruction pour l'executor de stocker le résultat ici
                        }),
                    });

                    edges.push(WorkflowEdge {
                        from: previous_node_id.clone(),
                        to: tool_node_id.clone(),
                        condition: None,
                    });

                    previous_node_id = tool_node_id;
                }

                let node_id = format!("gate_veto_{}", i);

                let mut params = json_value!({
                    "rule": veto.rule,
                    "action": veto.action
                });

                // Transmission de l'AST dynamique
                if let Some(ast) = &veto.ast {
                    if let Some(obj) = params.as_object_mut() {
                        obj.insert("ast".to_string(), ast.clone());
                    }
                }

                nodes.push(WorkflowNode {
                    id: node_id.clone(),
                    r#type: NodeType::GatePolicy,
                    name: format!("VETO: {}", veto.rule),
                    params,
                });

                edges.push(WorkflowEdge {
                    from: previous_node_id.clone(),
                    to: node_id.clone(),
                    condition: None,
                });

                previous_node_id = node_id;
            }
        }

        // 2.5 Injection de la Gouvernance Dynamique (WASM / Plugins)
        let wasm_node_id = "policy_wasm_check".to_string();
        nodes.push(WorkflowNode {
            id: wasm_node_id.clone(),
            r#type: NodeType::Wasm,
            name: "🛡️ Politique WASM (Hot-Swap)".into(),
            params: json_value!({}),
        });

        edges.push(WorkflowEdge {
            from: previous_node_id.clone(),
            to: wasm_node_id.clone(),
            condition: None,
        });
        previous_node_id = wasm_node_id;

        // 3. L'Agent d'Exécution
        let task_id = "agent_execution".to_string();
        nodes.push(WorkflowNode {
            id: task_id.clone(),
            r#type: NodeType::Task,
            name: format!("Exécution Stratégie {:?}", mandate.governance.strategy),
            params: json_value!({ "context_fetch": true }),
        });
        edges.push(WorkflowEdge {
            from: previous_node_id.clone(),
            to: task_id.clone(),
            condition: None,
        });
        previous_node_id = task_id;

        // 4. Le Consensus Algorithmique
        let vote_id = "consensus_condorcet".to_string();
        nodes.push(WorkflowNode {
            id: vote_id.clone(),
            r#type: NodeType::Decision,
            name: "Vote Condorcet Pondéré".into(),
            params: json_value!({
                "weights": mandate.governance.condorcet_weights,
                "threshold": 0.5
            }),
        });
        edges.push(WorkflowEdge {
            from: previous_node_id.clone(),
            to: vote_id.clone(),
            condition: None,
        });

        // 5. Fin
        nodes.push(WorkflowNode {
            id: "end".into(),
            r#type: NodeType::End,
            name: "Fin de Mission".into(),
            params: json_value!({}),
        });
        edges.push(WorkflowEdge {
            from: vote_id,
            to: "end".into(),
            condition: None,
        });

        WorkflowDefinition {
            id: wf_id,
            nodes,
            edges,
            entry: "start".into(),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow_engine::mandate::{
        Governance, HardLogic, Mandate, MandateMeta, Observability, Strategy, VetoRule,
    };

    fn build_test_mandate(rules: Vec<VetoRule>) -> Mandate {
        Mandate {
            id: "test_mandate_001".into(),
            meta: MandateMeta {
                author: "Admin".into(),
                status: "ACTIVE".into(),
                version: "v1".into(),
            },
            governance: Governance {
                strategy: Strategy::SafetyFirst,
                condorcet_weights: UnorderedMap::from([("sec".to_string(), 1.0)]),
            },
            hard_logic: HardLogic { vetos: rules },
            observability: Observability { heartbeat_ms: 100 },
            signature: None,
        }
    }

    #[test]
    fn test_compiler_dynamic_tool_injection() {
        let mandate = build_test_mandate(vec![
            VetoRule {
                rule: "VIBRATION_MAX".into(), // Doit injecter un outil
                active: true,
                action: "STOP".into(),
                ast: Some(json_value!({"Gt": [{"Var": "sensor_vibration"}, {"Val": 8.0}]})),
            },
            VetoRule {
                rule: "UNKNOWN_RULE".into(), // NE DOIT PAS injecter d'outil
                active: true,
                action: "LOG".into(),
                ast: Some(json_value!({"Eq": [{"Var": "x"}, {"Val": 1}]})),
            },
        ]);

        let wf = WorkflowCompiler::compile(&mandate);

        // Nœuds attendus :
        // 1. Start
        // 2. Tool (VIBRATION_MAX)
        // 3. Gate (VIBRATION_MAX)
        // 4. Gate (UNKNOWN_RULE) - Pas d'outil avant !
        // 5. WASM
        // 6. Agent Exec
        // 7. Vote
        // 8. End
        assert_eq!(
            wf.nodes.len(),
            8,
            "Le workflow doit avoir exactement 8 nœuds"
        );

        // Vérification de l'injection d'outil
        let tools: Vec<_> = wf
            .nodes
            .iter()
            .filter(|n| n.r#type == NodeType::CallMcp)
            .collect();
        assert_eq!(tools.len(), 1, "Un seul outil doit être injecté");
        assert_eq!(
            tools[0].params.get("output_key").unwrap().as_str().unwrap(),
            "sensor_vibration"
        );

        // Vérification des AST
        let gates: Vec<_> = wf
            .nodes
            .iter()
            .filter(|n| n.r#type == NodeType::GatePolicy)
            .collect();
        assert_eq!(gates.len(), 2, "Deux vetos actifs doivent être présents");
        assert!(gates[0].params.get("ast").is_some());
        assert!(gates[1].params.get("ast").is_some());
    }

    #[test]
    fn test_compiler_ignores_inactive_rules() {
        let mandate = build_test_mandate(vec![VetoRule {
            rule: "TEMP_MAX".into(),
            active: false, // Règle désactivée !
            action: "STOP".into(),
            ast: None,
        }]);

        let wf = WorkflowCompiler::compile(&mandate);
        let gates: Vec<_> = wf
            .nodes
            .iter()
            .filter(|n| n.r#type == NodeType::GatePolicy)
            .collect();
        let tools: Vec<_> = wf
            .nodes
            .iter()
            .filter(|n| n.r#type == NodeType::CallMcp)
            .collect();

        assert_eq!(
            gates.len(),
            0,
            "Aucun gate ne doit être généré pour un veto inactif"
        );
        assert_eq!(
            tools.len(),
            0,
            "Aucun outil ne doit être injecté pour un veto inactif"
        );
    }
}
