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

/// Type d'un nœud dans le graphe (correspond au schema JSON)
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Task,       // Tâche standard (Agent IA)
    Decision,   // Branchement conditionnel (Condorcet)
    Parallel,   // Exécution simultanée (Réservé pour v2)
    GateHitl,   // Pause pour validation humaine (Human In The Loop)
    GatePolicy, // Vérification automatique de règles (Vetos)
    CallMcp,    // Appel outil externe (Model Context Protocol)
    Wasm,       // Exécution d'un module WebAssembly (Sandboxé & Hot-swappable)
    End,        // Fin du flux
}

/// Statut d'exécution d'une instance ou d'un nœud
#[derive(Debug, Clone, Copy, Serializable, Deserializable, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionStatus {
    Pending,   // En attente
    Running,   // En cours
    Completed, // Terminé avec succès
    Failed,    // Erreur technique ou Veto
    Paused,    // En attente d'action humaine (HITL)
    Skipped,   // Branche non prise
}

/// Nœud unitaire du workflow
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct WorkflowNode {
    pub id: String,
    pub r#type: NodeType, // "type" est un mot clé réservé en Rust
    pub name: String,
    pub params: JsonValue, // Paramètres libres (JSON)
}

/// Lien orienté entre deux nœuds
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct WorkflowEdge {
    pub from: String,
    pub to: String,
    pub condition: Option<String>, // Script simple (ex: "status == 'ok'")
}

/// Définition statique du Workflow (le "Plan" compilé)
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct WorkflowDefinition {
    #[serde(rename = "_id")]
    pub id: String,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    pub entry: String, // ID du nœud de départ
}

/// Instance dynamique (l'Exécution en cours)
#[derive(Debug, Clone, Serializable, Deserializable)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowInstance {
    #[serde(rename = "_id")]
    pub id: String,
    pub workflow_id: String,
    pub status: ExecutionStatus,

    /// État de chaque nœud : NodeID -> Status
    pub node_states: UnorderedMap<String, ExecutionStatus>,

    /// Mémoire contextuelle du workflow (Variables partagées)
    pub context: UnorderedMap<String, JsonValue>,

    /// Journal d'audit de l'exécution
    pub logs: Vec<String>,

    pub created_at: i64,
    pub updated_at: i64,
}

impl WorkflowInstance {
    pub fn new(workflow_id: &str, context: UnorderedMap<String, JsonValue>) -> Self {
        Self {
            id: UniqueId::new_v4().to_string(),
            workflow_id: workflow_id.to_string(),
            status: ExecutionStatus::Pending,
            node_states: UnorderedMap::new(),
            context,
            logs: Vec::new(),
            created_at: UtcClock::now().timestamp(),
            updated_at: UtcClock::now().timestamp(),
        }
    }
}
