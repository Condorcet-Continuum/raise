// FICHIER : src-tauri/src/workflow_engine/state_machine.rs

use super::{ExecutionStatus, WorkflowDefinition, WorkflowInstance};
use crate::utils::Result;
use serde_json::Value; // Ajout nécessaire pour lire le contexte

pub struct WorkflowStateMachine {
    definition: WorkflowDefinition,
}

impl WorkflowStateMachine {
    pub fn new(definition: WorkflowDefinition) -> Self {
        Self { definition }
    }

    /// Détermine les prochains nœuds éligibles à l'exécution.
    /// Implémente la logique de synchronisation (attente de tous les parents)
    /// ET la logique de branchement conditionnel (filtrage des arcs).
    pub fn next_runnable_nodes(&self, instance: &WorkflowInstance) -> Vec<String> {
        let mut runnable = Vec::new();

        if instance.node_states.is_empty() {
            return vec![self.definition.entry.clone()];
        }

        for (node_id, status) in &instance.node_states {
            if *status == ExecutionStatus::Completed {
                // MODIFICATION : On récupère uniquement les enfants valides selon le contexte
                let children = self.get_valid_children(node_id, instance);

                for child_id in children {
                    // On vérifie que l'enfant n'est pas déjà lancé
                    // ET que tous ses parents (qui mènent à lui) sont satisfaits
                    if !instance.node_states.contains_key(&child_id)
                        && self.are_parents_satisfied(&child_id, instance)
                    {
                        runnable.push(child_id);
                    }
                }
            }
        }
        runnable
    }

    /// Effectue une transition d'état pour un nœud et met à jour le statut global.
    pub fn transition(
        &self,
        instance: &mut WorkflowInstance,
        node_id: &str,
        status: ExecutionStatus,
    ) -> Result<()> {
        instance.node_states.insert(node_id.to_string(), status);
        instance.updated_at = chrono::Utc::now().timestamp();

        // Gestion de l'échec critique
        if status == ExecutionStatus::Failed {
            instance.status = ExecutionStatus::Failed;
            instance
                .logs
                .push(format!("❌ Échec critique au nœud : {}", node_id));
        }

        // Vérification de la complétion globale du graphe
        if self.is_workflow_finished(instance) {
            if instance.status != ExecutionStatus::Failed {
                instance.status = ExecutionStatus::Completed;
            }
            instance.logs.push("🏁 Workflow terminé.".into());
        }

        Ok(())
    }

    /// Vérifie si tous les nœuds pointant vers child_id sont à l'état Completed.
    fn are_parents_satisfied(&self, child_id: &str, instance: &WorkflowInstance) -> bool {
        let parents: Vec<_> = self
            .definition
            .edges
            .iter()
            .filter(|edge| edge.to == child_id)
            .map(|edge| &edge.from)
            .collect();

        if parents.is_empty() {
            return true;
        }

        // Pour qu'un nœud démarre, tous ses parents définis dans le graphe doivent être terminés.
        // Note: Dans un branchement exclusif, cela implique que le noeud de jonction
        // ne doit pas avoir des parents de branches mutuellement exclusives (sinon il bloquera).
        parents
            .iter()
            .all(|p_id| instance.node_states.get(*p_id) == Some(&ExecutionStatus::Completed))
    }

    /// Récupère les enfants dont la condition de l'arc est validée
    fn get_valid_children(&self, node_id: &str, instance: &WorkflowInstance) -> Vec<String> {
        self.definition
            .edges
            .iter()
            .filter(|edge| edge.from == node_id)
            // Ici on applique le filtre de condition
            .filter(|edge| self.evaluate_edge_condition(&edge.condition, &instance.context))
            .map(|edge| edge.to.clone())
            .collect()
    }

    /// Évaluateur basique de condition (Syntaxe: "key == value")
    fn evaluate_edge_condition(
        &self,
        condition: &Option<String>,
        context: &std::collections::HashMap<String, Value>,
    ) -> bool {
        match condition {
            None => true, // Pas de condition = chemin par défaut
            Some(cond_str) => {
                // Parsing naïf : "variable == valeur"
                // Ex: "validation == 'approved'"
                let parts: Vec<&str> = cond_str.split("==").map(|s| s.trim()).collect();
                if parts.len() != 2 {
                    return false; // Syntaxe invalide
                }

                let key = parts[0];
                // Nettoyage des quotes autour de la valeur ('val' ou "val")
                let expected_val_str = parts[1].trim_matches('\'').trim_matches('"');

                // On cherche la variable dans le contexte
                if let Some(actual_val) = context.get(key) {
                    // Comparaison simple sous forme de string pour l'instant
                    let actual_str = match actual_val {
                        Value::String(s) => s.clone(),
                        Value::Bool(b) => b.to_string(),
                        Value::Number(n) => n.to_string(),
                        _ => return false,
                    };
                    return actual_str == expected_val_str;
                }

                false // Variable introuvable dans le contexte
            }
        }
    }

    fn is_workflow_finished(&self, instance: &WorkflowInstance) -> bool {
        if instance.status == ExecutionStatus::Paused {
            return false;
        }

        // Si plus rien n'est runnable et que rien n'est en cours, c'est fini.
        self.next_runnable_nodes(instance).is_empty()
            && !instance
                .node_states
                .values()
                .any(|s| *s == ExecutionStatus::Running)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow_engine::{NodeType, WorkflowDefinition, WorkflowEdge, WorkflowNode};
    use serde_json::json;
    use std::collections::HashMap;

    fn create_test_wf() -> WorkflowDefinition {
        // Graphe : A -> B, A -> C, B -> D, C -> D
        WorkflowDefinition {
            id: "diamond_test".into(),
            entry: "A".into(),
            nodes: vec![
                WorkflowNode {
                    id: "A".into(),
                    r#type: NodeType::Task,
                    name: "Start".into(),
                    params: json!({}),
                },
                WorkflowNode {
                    id: "B".into(),
                    r#type: NodeType::Task,
                    name: "Task B".into(),
                    params: json!({}),
                },
                WorkflowNode {
                    id: "C".into(),
                    r#type: NodeType::Task,
                    name: "Task C".into(),
                    params: json!({}),
                },
                WorkflowNode {
                    id: "D".into(),
                    r#type: NodeType::Task,
                    name: "Join D".into(),
                    params: json!({}),
                },
            ],
            edges: vec![
                WorkflowEdge {
                    from: "A".into(),
                    to: "B".into(),
                    condition: None,
                },
                WorkflowEdge {
                    from: "A".into(),
                    to: "C".into(),
                    condition: None,
                },
                WorkflowEdge {
                    from: "B".into(),
                    to: "D".into(),
                    condition: None,
                },
                WorkflowEdge {
                    from: "C".into(),
                    to: "D".into(),
                    condition: None,
                },
            ],
        }
    }

    #[test]
    fn test_diamond_join_logic() {
        let def = create_test_wf();
        let sm = WorkflowStateMachine::new(def);
        let mut instance = WorkflowInstance::new("diamond_test", HashMap::new());

        // A est fini
        sm.transition(&mut instance, "A", ExecutionStatus::Completed)
            .unwrap();

        // B et C sont prêts, mais pas D
        let runnable = sm.next_runnable_nodes(&instance);
        assert!(runnable.contains(&"B".to_string()));
        assert!(runnable.contains(&"C".to_string()));
        assert!(!runnable.contains(&"D".to_string()));

        // B finit, D attend toujours C
        sm.transition(&mut instance, "B", ExecutionStatus::Completed)
            .unwrap();
        assert!(!sm.next_runnable_nodes(&instance).contains(&"D".to_string()));

        // C finit, D devient enfin runnable
        sm.transition(&mut instance, "C", ExecutionStatus::Completed)
            .unwrap();
        assert!(sm.next_runnable_nodes(&instance).contains(&"D".to_string()));
    }

    #[test]
    fn test_failure_propagation() {
        let def = create_test_wf();
        let sm = WorkflowStateMachine::new(def);
        let mut instance = WorkflowInstance::new("fail_test", HashMap::new());

        // A échoue
        sm.transition(&mut instance, "A", ExecutionStatus::Failed)
            .unwrap();

        // Le statut global doit être Failed
        assert_eq!(instance.status, ExecutionStatus::Failed);

        // Plus rien ne doit être runnable (B et C attendent A Completed)
        assert!(sm.next_runnable_nodes(&instance).is_empty());
    }
}
