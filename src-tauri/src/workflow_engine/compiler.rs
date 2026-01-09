// FICHIER : src-tauri/src/workflow_engine/compiler.rs

use super::mandate::Mandate;
use super::{NodeType, WorkflowDefinition, WorkflowEdge, WorkflowNode};
use serde_json::json;

pub struct WorkflowCompiler;

impl WorkflowCompiler {
    /// Transforme un Mandat politique en Workflow technique exécutable
    pub fn compile(mandate: &Mandate) -> WorkflowDefinition {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let wf_id = format!("wf_{}_{}", mandate.meta.author, mandate.meta.version);

        // Pointeur vers le dernier nœud créé pour chaîner les edges
        let mut previous_node_id = "start".to_string();

        // 1. Nœud de Départ
        nodes.push(WorkflowNode {
            id: "start".into(),
            r#type: NodeType::Task,
            name: "Initialisation Mandat".into(),
            params: json!({
                "strategy": mandate.governance.strategy,
                "observability": mandate.observability
            }),
        });

        // 2. Compilation des Lignes Rouges (VETOS -> GatePolicy)
        for (i, veto) in mandate.hard_logic.vetos.iter().enumerate() {
            if veto.active {
                let node_id = format!("gate_veto_{}", i);

                nodes.push(WorkflowNode {
                    id: node_id.clone(),
                    r#type: NodeType::GatePolicy,
                    name: format!("VETO: {}", veto.rule),
                    params: json!({
                        "rule": veto.rule,
                        "action": veto.action
                    }),
                });

                edges.push(WorkflowEdge {
                    from: previous_node_id.clone(),
                    to: node_id.clone(),
                    condition: None,
                });

                previous_node_id = node_id;
            }
        }

        // 3. L'Agent d'Exécution
        let task_id = "agent_execution".to_string();
        nodes.push(WorkflowNode {
            id: task_id.clone(),
            r#type: NodeType::Task,
            name: format!("Exécution Stratégie {}", mandate.governance.strategy),
            params: json!({ "context_fetch": true }),
        });
        edges.push(WorkflowEdge {
            from: previous_node_id.clone(),
            to: task_id.clone(),
            condition: None,
        });
        previous_node_id = task_id;

        // 4. Le Consensus Algorithmique (Decision / Condorcet)
        let vote_id = "consensus_condorcet".to_string();
        nodes.push(WorkflowNode {
            id: vote_id.clone(),
            r#type: NodeType::Decision,
            name: "Vote Condorcet Pondéré".into(),
            params: json!({
                "weights": mandate.governance.condorcet_weights, // Injection des POIDS
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
            params: json!({}),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow_engine::mandate::{
        Governance, HardLogic, Mandate, MandateMeta, Observability, VetoRule,
    };
    use std::collections::HashMap;

    fn get_test_mandate() -> Mandate {
        Mandate {
            meta: MandateMeta {
                author: "Admin".into(),
                status: "ACTIVE".into(),
                version: "v1".into(),
            },
            governance: Governance {
                strategy: "SAFETY".into(),
                condorcet_weights: HashMap::from([
                    ("agent_security".to_string(), 3.0),
                    ("agent_finance".to_string(), 1.0),
                ]),
            },
            hard_logic: HardLogic {
                vetos: vec![
                    VetoRule {
                        rule: "VIBRATION_MAX".into(),
                        active: true,
                        action: "SHUTDOWN".into(),
                    },
                    VetoRule {
                        rule: "TEMP_MAX".into(),
                        active: false,
                        action: "LOG".into(),
                    },
                ],
            },
            observability: Observability {
                heartbeat_ms: 100,
                metrics: vec![],
            },
            signature: None,
        }
    }

    #[test]
    fn test_compiler_generates_workflow() {
        let mandate = get_test_mandate();
        let wf = WorkflowCompiler::compile(&mandate);

        assert_eq!(wf.id, "wf_Admin_v1");
        assert_eq!(wf.entry, "start");

        // 5 nœuds : Start, Veto(Active), Execution, Vote, End
        assert_eq!(wf.nodes.len(), 5);
        assert_eq!(wf.edges.len(), 4);

        // Vérification des poids injectés
        let decision_node = wf
            .nodes
            .iter()
            .find(|n| n.r#type == NodeType::Decision)
            .unwrap();
        let weights = decision_node.params.get("weights").unwrap();

        assert_eq!(weights.get("agent_security").unwrap().as_f64(), Some(3.0));
    }
}
