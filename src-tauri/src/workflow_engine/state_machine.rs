// FICHIER : src-tauri/src/workflow_engine/state_machine.rs
use super::{ExecutionStatus, WorkflowDefinition, WorkflowInstance};
use crate::utils::prelude::*;

/// Moteur de règles de transition pour le workflow.
/// C'est lui qui décide quel nœud doit s'exécuter ensuite.
pub struct WorkflowStateMachine {
    definition: WorkflowDefinition,
}

impl WorkflowStateMachine {
    pub fn new(definition: WorkflowDefinition) -> Self {
        Self { definition }
    }

    /// Détermine la liste des ID de nœuds qui peuvent être exécutés maintenant.
    pub fn next_runnable_nodes(&self, instance: &WorkflowInstance) -> Vec<String> {
        let mut runnable = Vec::new();

        // Si le workflow est en pause ou terminé, rien ne bouge
        if instance.status == ExecutionStatus::Paused
            || instance.status == ExecutionStatus::Completed
            || instance.status == ExecutionStatus::Failed
        {
            return runnable;
        }

        for node in &self.definition.nodes {
            let node_id = &node.id;

            // 1. Si le nœud est déjà traité (Completed, Failed, Skipped), on passe
            if let Some(status) = instance.node_states.get(node_id) {
                if *status != ExecutionStatus::Pending && *status != ExecutionStatus::Running {
                    continue;
                }
                // Si Running, on ne le relance pas (sauf logique de retry, non implémentée ici)
                if *status == ExecutionStatus::Running {
                    continue;
                }
            }

            // 2. Vérification des Parents (Dépendances)
            let parents = self.get_parents(node_id);

            // Cas spécial : Le nœud de départ n'a pas de parents
            if parents.is_empty() {
                if node_id == &self.definition.entry && !instance.node_states.contains_key(node_id)
                {
                    runnable.push(node_id.clone());
                }
                continue;
            }

            // 3. Logique de Synchronisation (Tous les parents doivent être terminés)
            let mut all_parents_done = true;
            let mut parent_failed = false;

            for parent_id in &parents {
                match instance.node_states.get(parent_id) {
                    Some(ExecutionStatus::Completed) => {
                        // Le parent est OK, mais l'arc a-t-il une condition ?
                        if !self.check_transition_condition(parent_id, node_id, instance) {
                            // Parent OK mais condition non remplie => Ce chemin est fermé
                            all_parents_done = false;
                            break;
                        }
                    }
                    Some(ExecutionStatus::Skipped) => {
                        // Si un parent est skippé, l'enfant est skippé aussi (propagation)
                        // (Géré lors de la transition, ici on considère juste qu'on ne peut pas run)
                        all_parents_done = false;
                        break;
                    }
                    Some(ExecutionStatus::Failed) => {
                        parent_failed = true;
                        all_parents_done = false;
                        break;
                    }
                    _ => {
                        // Parent Pending/Running/Paused
                        all_parents_done = false;
                        break;
                    }
                }
            }

            if parent_failed {
                // Si un parent (ex: Veto) a échoué, ce nœud ne s'exécutera jamais.
                // Idéalement, on devrait le marquer Skipped ou Failed ici,
                // mais next_runnable ne fait que de la lecture.
                continue;
            }

            if all_parents_done {
                runnable.push(node_id.clone());
            }
        }

        runnable
    }

    /// Applique le changement d'état après l'exécution d'un nœud
    pub fn transition(
        &self,
        instance: &mut WorkflowInstance,
        node_id: &str,
        new_status: ExecutionStatus,
    ) -> Result<()> {
        // Mise à jour de l'état du nœud
        instance.node_states.insert(node_id.to_string(), new_status);

        // Gestion de la fin globale ou de l'échec
        if new_status == ExecutionStatus::Failed {
            // Si c'est un Veto critique, tout le workflow échoue
            // (Sauf si on avait une logique de try/catch, absente pour l'instant)
            tracing::error!("❌ Nœud {} échoué -> Arrêt du Workflow", node_id);
            instance.status = ExecutionStatus::Failed;
            return Ok(());
        }

        // Vérifier si c'était le dernier nœud
        if self.is_end_node(node_id) {
            tracing::info!("🏁 Fin du Workflow atteinte par le nœud {}", node_id);
            instance.status = ExecutionStatus::Completed;
        }

        // TODO: Propagation du statut "Skipped" aux enfants des branches non prises
        // Ce serait ici qu'on invaliderait les chemins alternatifs d'un Decision.

        Ok(())
    }

    // --- Helpers ---

    fn get_parents(&self, node_id: &str) -> Vec<String> {
        self.definition
            .edges
            .iter()
            .filter(|e| e.to == node_id)
            .map(|e| e.from.clone())
            .collect()
    }

    fn is_end_node(&self, node_id: &str) -> bool {
        // Un nœud est final s'il n'a pas d'enfants sortants
        // OU s'il est explicitement de type "End" (vérifié via la definition)
        if let Some(node) = self.definition.nodes.iter().find(|n| n.id == node_id) {
            if matches!(node.r#type, super::NodeType::End) {
                return true;
            }
        }

        !self.definition.edges.iter().any(|e| e.from == node_id)
    }

    /// Vérifie si la condition portée par l'arc (Edge) est valide
    fn check_transition_condition(
        &self,
        from: &str,
        to: &str,
        instance: &WorkflowInstance,
    ) -> bool {
        let edge = self
            .definition
            .edges
            .iter()
            .find(|e| e.from == from && e.to == to);

        if let Some(e) = edge {
            if let Some(condition_script) = &e.condition {
                return self.evaluate_condition(condition_script, &instance.context);
            }
        }

        // Pas de condition = toujours vrai
        true
    }

    /// Évaluateur basique de condition (Moteur symbolique simple)
    /// Supporte: "var == 'valeur'"
    fn evaluate_condition(
        &self,
        script: &str,
        context: &std::collections::HashMap<String, Value>,
    ) -> bool {
        // Parsing naïf pour le prototype : "variable == 'valeur'"
        // Pour la prod : utiliser une lib comme `rhai` ou `json_logic`

        if script.contains("==") {
            let parts: Vec<&str> = script.split("==").collect();
            if parts.len() == 2 {
                let key = parts[0].trim();
                let target_val_str = parts[1].trim().replace("'", "").replace("\"", "");

                if let Some(actual_val) = context.get(key) {
                    // Comparaison String
                    if let Some(s) = actual_val.as_str() {
                        return s == target_val_str;
                    }
                    // Comparaison Bool
                    if let Some(b) = actual_val.as_bool() {
                        return b.to_string() == target_val_str;
                    }
                }
                return false; // Clé manquante ou type incompatible
            }
        }

        // Si script non reconnu, on retourne false par sécurité (Fail-Safe)
        tracing::warn!("⚠️ Script de condition non supporté : {}", script);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow_engine::{NodeType, WorkflowEdge, WorkflowNode};
    use serde_json::json;
    use std::collections::HashMap;

    fn create_dummy_def() -> WorkflowDefinition {
        WorkflowDefinition {
            id: "wf_1".into(),
            entry: "start".into(),
            nodes: vec![
                WorkflowNode {
                    id: "start".into(),
                    r#type: NodeType::Task,
                    name: "S".into(),
                    params: json!({}),
                },
                WorkflowNode {
                    id: "mid".into(),
                    r#type: NodeType::Task,
                    name: "M".into(),
                    params: json!({}),
                },
                WorkflowNode {
                    id: "end".into(),
                    r#type: NodeType::End,
                    name: "E".into(),
                    params: json!({}),
                },
            ],
            edges: vec![
                WorkflowEdge {
                    from: "start".into(),
                    to: "mid".into(),
                    condition: None,
                },
                WorkflowEdge {
                    from: "mid".into(),
                    to: "end".into(),
                    condition: None,
                },
            ],
        }
    }

    #[test]
    fn test_sequential_flow() {
        let def = create_dummy_def();
        let sm = WorkflowStateMachine::new(def);
        let mut instance = WorkflowInstance::new("wf_1", HashMap::new());

        // 1. Initial : Start doit être runnable
        let next = sm.next_runnable_nodes(&instance);
        assert_eq!(next, vec!["start"]);

        // 2. Start exécuté
        sm.transition(&mut instance, "start", ExecutionStatus::Completed)
            .unwrap();

        // 3. Mid doit être runnable
        let next = sm.next_runnable_nodes(&instance);
        assert_eq!(next, vec!["mid"]);
    }

    #[test]
    fn test_conditional_branching_simple() {
        let def = WorkflowDefinition {
            id: "wf_branch".into(),
            entry: "start".into(),
            nodes: vec![
                WorkflowNode {
                    id: "start".into(),
                    r#type: NodeType::Task,
                    name: "S".into(),
                    params: json!({}),
                },
                WorkflowNode {
                    id: "path_a".into(),
                    r#type: NodeType::Task,
                    name: "A".into(),
                    params: json!({}),
                },
            ],
            edges: vec![WorkflowEdge {
                from: "start".into(),
                to: "path_a".into(),
                condition: Some("status == 'ok'".into()),
            }],
        };
        let sm = WorkflowStateMachine::new(def);

        // Cas A : Condition remplie
        let mut ctx_ok = HashMap::new();
        ctx_ok.insert("status".into(), json!("ok"));
        let mut inst_ok = WorkflowInstance::new("wf_branch", ctx_ok);
        inst_ok
            .node_states
            .insert("start".into(), ExecutionStatus::Completed);

        assert_eq!(sm.next_runnable_nodes(&inst_ok), vec!["path_a"]);

        // Cas B : Condition non remplie
        let mut ctx_ko = HashMap::new();
        ctx_ko.insert("status".into(), json!("error"));
        let mut inst_ko = WorkflowInstance::new("wf_branch", ctx_ko);
        inst_ko
            .node_states
            .insert("start".into(), ExecutionStatus::Completed);

        assert!(sm.next_runnable_nodes(&inst_ko).is_empty());
    }
}
