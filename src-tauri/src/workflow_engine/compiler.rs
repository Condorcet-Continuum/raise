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
        // On nettoie le nom pour l'ID
        let wf_id = format!(
            "wf_{}_{}",
            mandate.meta.author.replace(" ", ""),
            mandate.meta.version
        );

        // Pointeur vers le dernier nœud créé pour chaîner les edges
        let mut previous_node_id = "start".to_string();

        // 1. Nœud de Départ
        nodes.push(WorkflowNode {
            id: "start".into(),
            r#type: NodeType::Task, // Changé temporairement en Task pour initialiser
            name: "Initialisation Mandat".into(),
            params: json!({
                "strategy": mandate.governance.strategy,
                "observability": mandate.observability
            }),
        });

        // 2. Compilation des Lignes Rouges (VETOS -> GatePolicy)
        for (i, veto) in mandate.hard_logic.vetos.iter().enumerate() {
            if veto.active {
                // --- Injection de l'outil de lecture AVANT le Veto ---
                if veto.rule == "VIBRATION_MAX" {
                    let tool_node_id = format!("tool_read_vibration_{}", i);

                    nodes.push(WorkflowNode {
                        id: tool_node_id.clone(),
                        r#type: NodeType::CallMcp, // Action : Lire le capteur
                        name: "Lecture Capteur Vibration".into(),
                        params: json!({
                            "tool_name": "read_system_metrics",
                            "arguments": { "sensor_id": "vibration_z" }
                        }),
                    });

                    // Lien : Précédent -> Outil
                    edges.push(WorkflowEdge {
                        from: previous_node_id.clone(),
                        to: tool_node_id.clone(),
                        condition: None,
                    });

                    // Le nœud précédent devient l'outil
                    previous_node_id = tool_node_id;
                }

                let node_id = format!("gate_veto_{}", i);

                nodes.push(WorkflowNode {
                    id: node_id.clone(),
                    r#type: NodeType::GatePolicy, // Contrôle : Vérifier la valeur
                    name: format!("VETO: {}", veto.rule),
                    params: json!({
                        "rule": veto.rule,
                        "action": veto.action
                    }),
                });

                // Lien : (Précédent ou Outil) -> GatePolicy
                edges.push(WorkflowEdge {
                    from: previous_node_id.clone(),
                    to: node_id.clone(),
                    condition: None,
                });

                previous_node_id = node_id;
            }
        }

        // --- NOUVEAU : 2.5 Injection de la Gouvernance Dynamique (WASM) ---
        // On insère ce nœud entre les Vetos (Hard Logic) et l'exécution (Agents)
        let wasm_node_id = "policy_wasm_check".to_string();
        nodes.push(WorkflowNode {
            id: wasm_node_id.clone(),
            r#type: NodeType::Wasm, // Hot-Swap dynamique
            name: "🛡️ Politique WASM (Hot-Swap)".into(),
            params: json!({}), // Utilise le path par défaut défini dans l'executor
        });

        edges.push(WorkflowEdge {
            from: previous_node_id.clone(),
            to: wasm_node_id.clone(),
            condition: None,
        });
        previous_node_id = wasm_node_id;
        // ------------------------------------------------------------------

        // 3. L'Agent d'Exécution
        let task_id = "agent_execution".to_string();
        nodes.push(WorkflowNode {
            id: task_id.clone(),
            r#type: NodeType::Task,
            name: format!("Exécution Stratégie {:?}", mandate.governance.strategy),
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
        Governance, HardLogic, Mandate, MandateMeta, Observability, Strategy, VetoRule,
    };
    use std::collections::HashMap;

    fn get_test_mandate() -> Mandate {
        Mandate {
            id: "test_mandate_001".into(),
            meta: MandateMeta {
                author: "Admin".into(),
                status: "ACTIVE".into(),
                version: "v1".into(),
            },
            governance: Governance {
                strategy: Strategy::SafetyFirst,
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

        // On doit avoir 7 nœuds maintenant :
        // 1. Start
        // 2. ToolRead (pour VIBRATION_MAX)
        // 3. GateVeto (pour VIBRATION_MAX)
        // 4. WASM (Politique Dynamique) <-- AJOUTÉ
        // 5. Exec (Agent)
        // 6. Vote (Condorcet)
        // 7. End
        assert_eq!(
            wf.nodes.len(),
            7,
            "Le nombre de nœuds doit inclure le nœud WASM"
        );

        // Vérifions que le nœud WASM est bien présent
        let wasm_node = wf.nodes.iter().find(|n| n.r#type == NodeType::Wasm);
        assert!(wasm_node.is_some(), "Le nœud WASM doit être injecté");

        // Vérifions que le nœud CallMcp est bien présent
        let tool_node = wf.nodes.iter().find(|n| n.r#type == NodeType::CallMcp);
        assert!(tool_node.is_some(), "Le nœud outil doit être injecté");

        let decision_node = wf
            .nodes
            .iter()
            .find(|n| n.r#type == NodeType::Decision)
            .unwrap();
        let weights = decision_node.params.get("weights").unwrap();

        assert_eq!(weights.get("agent_security").unwrap().as_f64(), Some(3.0));
    }
}
