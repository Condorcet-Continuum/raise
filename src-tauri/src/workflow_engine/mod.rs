// FICHIER : src-tauri/src/workflow_engine/mod.rs

pub mod compiler;
pub mod critic;
pub mod executor;
pub mod handlers;
pub mod mandate;
pub mod scheduler;
pub mod state_machine;
pub mod tools;

use crate::utils::prelude::*;

// --- RE-EXPORTS (L'API Publique du Moteur) ---
pub use compiler::WorkflowCompiler;
pub use executor::WorkflowExecutor;
pub use mandate::Mandate;
pub use scheduler::WorkflowScheduler;
pub use state_machine::WorkflowStateMachine;

/// Type d'un nœud dans le graphe (aligné avec les besoins MBSE)
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Task,        // Tâche standard (Phase ou action déléguée à une Squad IA)
    Decision,    // Branchement conditionnel (Condorcet ou Rule Engine)
    Parallel,    // Exécution simultanée de plusieurs tâches/agents
    GateHitl,    // Validation humaine (Human In The Loop) pour approuver une phase
    QualityGate, // (Ex-GatePolicy) Vérification auto via les QualityRules (AST)
    CallMcp,     // Appel outil externe direct (Model Context Protocol)
    Wasm,        // Exécution d'un module WebAssembly
    Milestone,   // NOUVEAU: Jalon bloquant marquant la fin d'une phase majeure
    SubProject,  // NOUVEAU: Appel à un autre workflow (Sous-graphe)
    End,         // Fin du flux
}

/// Statut d'exécution d'une instance ou d'un nœud
#[derive(Debug, Clone, Copy, Serializable, Deserializable, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionStatus {
    Pending,   // En attente
    Running,   // En cours d'exécution par la Squad
    Completed, // Terminé avec succès (Validé par QA/HITL)
    Failed,    // Erreur technique ou rejet qualité
    Paused,    // En attente d'action humaine (HITL)
    Skipped,   // Branche non prise
    Blocked,   // NOUVEAU: Bloqué en attente d'une dépendance externe
    InReview,  // NOUVEAU: En cours d'audit (Critic/QualityGate)
}

/// Nœud unitaire du workflow (Le Graphe)
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct WorkflowNode {
    pub id: String,
    pub r#type: NodeType,
    pub name: String,
    pub params: JsonValue, // Contient dynamiquement les directives de la tâche
}

/// Lien orienté entre deux nœuds
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct WorkflowEdge {
    pub from: String,
    pub to: String,
    pub condition: Option<String>, // Expression AST pour le Rules Engine
}

/// Définition statique du Workflow (Le "Template" ou "Plan" compilé)
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct WorkflowDefinition {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub _id: Option<String>,
    pub handle: String,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    pub entry: String, // ID du nœud de départ
}

/// Instance dynamique (L'Exécution en cours - Jumeau Numérique)
/// Aligné sur workflow-instance.schema.json
#[derive(Debug, Clone, Serializable, Deserializable)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowInstance {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub _id: Option<String>,
    pub handle: String,      // Slug unique
    pub workflow_id: String, // ID du WorkflowDefinition
    pub mission_id: String,  // UUID de la mission métier
    pub status: ExecutionStatus,

    /// État de chaque nœud : NodeID -> Status
    pub node_states: UnorderedMap<String, ExecutionStatus>,

    /// Mémoire contextuelle (Jumeau Numérique / Données MBSE partagées)
    pub context: UnorderedMap<String, JsonValue>,

    /// Traces d'explicabilité générées par l'IA (UUIDs vers XaiFrame)
    pub xai_traces: Vec<String>,

    /// Journal d'audit détaillé
    pub logs: Vec<String>,

    pub created_at: i64,
    pub updated_at: i64,
}

impl WorkflowInstance {
    pub fn new(
        handle: &str,
        workflow_id: &str,
        mission_id: &str,
        initial_context: UnorderedMap<String, JsonValue>,
    ) -> Self {
        Self {
            _id: None,
            handle: handle.to_string(),
            workflow_id: workflow_id.to_string(),
            mission_id: mission_id.to_string(),
            status: ExecutionStatus::Pending,
            node_states: UnorderedMap::new(),
            context: initial_context,
            xai_traces: Vec::new(),
            logs: vec![format!(
                "Création de l'instance pour la mission {}",
                mission_id
            )],
            created_at: UtcClock::now().timestamp(),
            updated_at: UtcClock::now().timestamp(),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_type_serialization() {
        // Vérifie que les types sont bien sérialisés en snake_case pour JSON-DB
        let t1 = NodeType::GateHitl;
        let json_t1 = json::serialize_to_string(&t1).unwrap();
        assert_eq!(json_t1, "\"gate_hitl\"");

        let t2 = NodeType::QualityGate;
        let json_t2 = json::serialize_to_string(&t2).unwrap();
        assert_eq!(json_t2, "\"quality_gate\"");
    }

    #[test]
    fn test_execution_status_serialization() {
        // Vérifie la sérialisation en SCREAMING_SNAKE_CASE
        let s1 = ExecutionStatus::InReview;
        let json_s1 = json::serialize_to_string(&s1).unwrap();
        assert_eq!(json_s1, "\"IN_REVIEW\"");

        let s2 = ExecutionStatus::Paused;
        let json_s2 = json::serialize_to_string(&s2).unwrap();
        assert_eq!(json_s2, "\"PAUSED\"");
    }

    #[test]
    fn test_workflow_instance_initialization() {
        let handle = "mission-apollo-11-exec";
        let wf_id = "wf_template_vcycle";
        let mission_id = "uuid-mission-1234";

        let mut initial_ctx = UnorderedMap::new();
        initial_ctx.insert("budget".into(), json_value!(5000));

        let instance = WorkflowInstance::new(handle, wf_id, mission_id, initial_ctx);

        assert_eq!(instance.handle, handle);
        assert_eq!(instance.workflow_id, wf_id);
        assert_eq!(instance.mission_id, mission_id);
        assert_eq!(instance.status, ExecutionStatus::Pending);

        // Vérifie que le contexte est bien injecté
        assert_eq!(
            instance.context.get("budget").unwrap().as_i64().unwrap(),
            5000
        );

        // Les collections de traçabilité doivent être vides
        assert!(instance.xai_traces.is_empty());
        assert!(instance.node_states.is_empty());
        assert_eq!(instance.logs.len(), 1); // 1 log de création
    }
}
