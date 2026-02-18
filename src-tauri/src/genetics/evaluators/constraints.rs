use super::architecture::ArchitectureCostModel;
use crate::genetics::genomes::arcadia_arch::SystemAllocationGenome;
use crate::utils::fmt::Debug;

/// Trait pour définir une règle métier qui doit être respectée.
/// Retourne un score de violation (0.0 = Respecté, >0.0 = Violé).
pub trait SystemConstraint: Send + Sync + Debug {
    fn check(&self, genome: &SystemAllocationGenome, model: &ArchitectureCostModel) -> f32;
    fn name(&self) -> String;
}

/// Contrainte de Capacité (Hard Constraint classique).
/// Vérifie qu'aucun composant n'est surchargé (CPU/RAM/etc).
#[derive(Debug)]
pub struct CapacityConstraint {
    pub penalty_factor: f32, // Poids de la pénalité (ex: 10.0 per unit overflow)
}

impl CapacityConstraint {
    pub fn new(penalty_factor: f32) -> Self {
        Self { penalty_factor }
    }
}

impl SystemConstraint for CapacityConstraint {
    fn name(&self) -> String {
        "Capacity Constraint".to_string()
    }

    fn check(&self, genome: &SystemAllocationGenome, model: &ArchitectureCostModel) -> f32 {
        let num_components = model.component_capacities.len();
        let mut loads = vec![0.0; num_components];
        let mut total_violation = 0.0;

        // 1. Calculer la charge cumulée
        for (func_idx, &comp_idx) in genome.genes.iter().enumerate() {
            if comp_idx < num_components {
                loads[comp_idx] += model.function_loads[func_idx];
            } else {
                total_violation += 1000.0; // Index hors limites (ne devrait pas arriver)
            }
        }

        // 2. Vérifier dépassement
        for (comp_idx, load) in loads.iter().enumerate() {
            let capacity = model.component_capacities[comp_idx];
            if *load > capacity {
                let overflow = load - capacity;
                total_violation += overflow * self.penalty_factor;
            }
        }

        total_violation
    }
}

/// Contrainte de Ségrégation (Safety).
/// Interdit à deux fonctions critiques d'être sur le même composant.
#[derive(Debug)]
pub struct SegregationConstraint {
    pub func_a_idx: usize,
    pub func_b_idx: usize,
    pub penalty: f32,
}

impl SystemConstraint for SegregationConstraint {
    fn name(&self) -> String {
        format!("Segregation(F{}, F{})", self.func_a_idx, self.func_b_idx)
    }

    fn check(&self, genome: &SystemAllocationGenome, _model: &ArchitectureCostModel) -> f32 {
        if self.func_a_idx >= genome.genes.len() || self.func_b_idx >= genome.genes.len() {
            return 0.0; // Ignorer si IDs invalides
        }

        let comp_a = genome.genes[self.func_a_idx];
        let comp_b = genome.genes[self.func_b_idx];

        if comp_a == comp_b {
            self.penalty // Violation ! Elles sont au même endroit
        } else {
            0.0
        }
    }
}

/// Contrainte de Co-Localisation (Performance/Safety).
/// Force deux fonctions à être sur le même composant (ex: latence ultra-faible requise).
#[derive(Debug)]
pub struct ColocationConstraint {
    pub func_a_idx: usize,
    pub func_b_idx: usize,
    pub penalty: f32,
}

impl SystemConstraint for ColocationConstraint {
    fn name(&self) -> String {
        format!("CoLocation(F{}, F{})", self.func_a_idx, self.func_b_idx)
    }

    fn check(&self, genome: &SystemAllocationGenome, _model: &ArchitectureCostModel) -> f32 {
        if self.func_a_idx >= genome.genes.len() || self.func_b_idx >= genome.genes.len() {
            return 0.0;
        }

        let comp_a = genome.genes[self.func_a_idx];
        let comp_b = genome.genes[self.func_b_idx];

        if comp_a != comp_b {
            self.penalty // Violation ! Elles sont séparées
        } else {
            0.0
        }
    }
}

/// Contrainte de Composant Interdit (Blacklist).
/// Une fonction spécifique ne DOIT PAS aller sur un composant spécifique (ex: driver GPU sur CPU lent).
#[derive(Debug)]
pub struct ForbiddenPlacementConstraint {
    pub func_idx: usize,
    pub forbidden_comp_idx: usize,
    pub penalty: f32,
}

impl SystemConstraint for ForbiddenPlacementConstraint {
    fn name(&self) -> String {
        format!(
            "Forbidden(F{} -> C{})",
            self.func_idx, self.forbidden_comp_idx
        )
    }

    fn check(&self, genome: &SystemAllocationGenome, _model: &ArchitectureCostModel) -> f32 {
        if self.func_idx >= genome.genes.len() {
            return 0.0;
        }

        if genome.genes[self.func_idx] == self.forbidden_comp_idx {
            self.penalty
        } else {
            0.0
        }
    }
}

// --- Tests Unitaires ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::genetics::evaluators::architecture::ArchitectureCostModel;
    use crate::genetics::genomes::arcadia_arch::SystemAllocationGenome;

    fn mock_model() -> ArchitectureCostModel {
        ArchitectureCostModel::new(
            3,
            2,                                // 3 Fonctions, 2 Composants
            &[],                              // Pas de flux
            &[(0, 10.0), (1, 5.0), (2, 5.0)], // Loads: F0=10, F1=5, F2=5
            &[(0, 12.0), (1, 10.0)],          // Caps: C0=12, C1=10
        )
    }

    fn mock_genome(genes: Vec<usize>) -> SystemAllocationGenome {
        SystemAllocationGenome {
            genes,
            function_ids: vec![],
            available_component_ids: vec![],
        }
    }

    #[test]
    fn test_capacity_constraint() {
        let model = mock_model();
        let constraint = CapacityConstraint::new(1.0);

        // Cas Valide: F0(10)->C0(12), F1(5)->C1(10)
        let g_valid = mock_genome(vec![0, 1, 1]);
        assert_eq!(constraint.check(&g_valid, &model), 0.0);

        // Cas Invalide: Tout sur C1(10). Total Load = 20. Violation = 10.
        let g_invalid = mock_genome(vec![1, 1, 1]);
        assert_eq!(constraint.check(&g_invalid, &model), 10.0);
    }

    #[test]
    fn test_segregation_constraint() {
        let model = mock_model();
        let constraint = SegregationConstraint {
            func_a_idx: 0,
            func_b_idx: 1,
            penalty: 100.0,
        };

        // F0 et F1 sur C0 -> Violation
        let g_same = mock_genome(vec![0, 0, 1]);
        assert_eq!(constraint.check(&g_same, &model), 100.0);

        // F0 sur C0, F1 sur C1 -> OK
        let g_diff = mock_genome(vec![0, 1, 1]);
        assert_eq!(constraint.check(&g_diff, &model), 0.0);
    }
}
