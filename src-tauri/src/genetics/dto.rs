use crate::utils::prelude::*;

// --- Configuration & Entrées ---

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OptimizationRequest {
    // Configuration de l'algo
    pub population_size: usize,
    pub max_generations: usize,
    pub mutation_rate: f32,
    pub crossover_rate: f32,

    // Données du Modèle Arcadia
    pub functions: Vec<FunctionInfo>,
    pub components: Vec<ComponentInfo>,
    pub flows: Vec<DataFlowInfo>,

    // Contraintes optionnelles
    pub constraints: Option<ConstraintConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FunctionInfo {
    pub id: String,
    pub load: f32, // Coût CPU/RAM
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ComponentInfo {
    pub id: String,
    pub capacity: f32, // Capacité Max
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DataFlowInfo {
    pub source_id: String,
    pub target_id: String,
    pub volume: f32, // Poids de l'échange
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ConstraintConfig {
    pub capacity_penalty: f32,
    pub segregations: Vec<(String, String)>, // Paires d'IDs à séparer
}

// --- Sorties & Feedback ---

#[derive(Debug, Serialize, Clone)]
pub struct OptimizationProgress {
    pub generation: usize,
    pub best_fitness: Vec<f32>, // [Coupling, Balance]
    pub diversity: f32,         // Crowding distance avg
}

#[derive(Debug, Serialize, Clone)]
pub struct OptimizationResult {
    pub duration_ms: u128,
    pub pareto_front: Vec<AllocatedSolution>,
}

#[derive(Debug, Serialize, Clone)]
pub struct AllocatedSolution {
    pub fitness: Vec<f32>,
    pub constraint_violation: f32,
    pub allocation: Vec<(String, String)>, // [(FuncID, CompID)]
}
