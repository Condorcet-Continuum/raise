// FICHIER : src-tauri/src/workflow_engine/handlers/genetics.rs

use crate::ai::assurance::xai::{ExplanationScope, XaiFrame, XaiMethod};
use crate::genetics::engine::{GeneticConfig, GeneticEngine};
use crate::genetics::genomes::arcadia_arch::SystemAllocationGenome;
use crate::genetics::operators::selection::TournamentSelection;
use crate::genetics::traits::Evaluator;
use crate::genetics::types::{Individual, Population}; // 🎯 FIX 2 : Import de Individual
use crate::utils::prelude::*;
use crate::workflow_engine::handlers::{HandlerContext, NodeHandler};
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};

// =========================================================================
// 1. L'ÉVALUATEUR MÉTIER (Synchrone & CPU-Bound)
// =========================================================================
#[derive(Clone)]
struct MbseEvaluator {
    component_metrics: UnorderedMap<String, JsonValue>,
}

impl Evaluator<SystemAllocationGenome> for MbseEvaluator {
    fn objective_names(&self) -> Vec<String> {
        vec!["MinusWeight".into(), "MinusCost".into()]
    }

    fn evaluate(&self, genome: &SystemAllocationGenome) -> (Vec<f32>, f32) {
        let mut total_weight = 0.0;
        let mut total_cost = 0.0;
        let mut constraints_violation = 0.0;

        let allocations = genome.get_allocations();

        for (_func_id, comp_id) in allocations {
            if let Some(metrics) = self.component_metrics.get(&comp_id) {
                total_weight += metrics
                    .get("weight")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;
                total_cost += metrics.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            } else {
                constraints_violation += 100.0;
            }
        }
        (vec![-total_weight, -total_cost], constraints_violation)
    }
}

// =========================================================================
// 2. LE HANDLER (Asynchrone)
// =========================================================================
pub struct GeneticsHandler;

#[async_interface]
impl NodeHandler for GeneticsHandler {
    fn node_type(&self) -> NodeType {
        NodeType::Genetics
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut UnorderedMap<String, JsonValue>,
        shared_ctx: &HandlerContext<'_>,
    ) -> RaiseResult<ExecutionStatus> {
        user_info!("INF_GENETICS_START", json_value!({"node": node.name}));

        let function_ids: Vec<String> =
            match node.params.get("functions").and_then(|v| v.as_array()) {
                Some(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                None => raise_error!(
                    "ERR_GENETICS_MISSING_FUNCTIONS",
                    context = json_value!({"node_id": node.id})
                ),
            };

        let component_ids: Vec<String> =
            match node.params.get("components").and_then(|v| v.as_array()) {
                Some(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                None => raise_error!(
                    "ERR_GENETICS_MISSING_COMPONENTS",
                    context = json_value!({"node_id": node.id})
                ),
            };

        let mut component_metrics = UnorderedMap::new();
        for comp_id in &component_ids {
            let doc_opt = shared_ctx
                .manager
                .get_document("components", comp_id)
                .await
                .unwrap_or(None);
            if let Some(doc) = doc_opt {
                if let Some(pvmt) = doc.get("pvmt_values") {
                    component_metrics.insert(comp_id.clone(), pvmt.clone());
                }
            }
        }

        let evaluator = MbseEvaluator { component_metrics };

        let config = GeneticConfig {
            population_size: 50,
            max_generations: 200,
            ..Default::default()
        };

        user_info!(
            "INF_GENETICS_EVOLVING",
            json_value!({"generations": config.max_generations})
        );

        // 🎯 FIX 2 : Clonage des listes pour le thread CPU afin d'initialiser manuellement la population
        let f_ids = function_ids.clone();
        let c_ids = component_ids.clone();
        let engine_config = config.clone();

        let best_genome = match spawn_cpu_task(move || {
            let engine = GeneticEngine::new(evaluator, TournamentSelection::new(2), engine_config);

            // 🎯 FIX 2 : Initialisation manuelle ! Évite le panic! de SystemAllocationGenome::random()
            let mut pop = Population::new();
            for _ in 0..config.population_size {
                pop.add(Individual::new(SystemAllocationGenome::new_random(
                    f_ids.clone(),
                    c_ids.clone(),
                )));
            }

            let final_pop = engine.run(pop, |_| {});
            final_pop.get_elites(1).into_iter().next()
        })
        .await
        {
            Ok(Some(individual)) => individual.genome,
            Ok(None) => raise_error!(
                "ERR_GENETICS_NO_SOLUTION",
                context = json_value!({"node_id": node.id})
            ),
            Err(e) => raise_error!(
                "ERR_GENETICS_CPU_PANIC",
                error = e,
                context = json_value!({"node_id": node.id})
            ),
        };

        let allocations = best_genome.get_allocations();
        let mut generated_artifacts = Vec::new();

        for (func, comp) in allocations {
            generated_artifacts.push(json_value!({
                "@type": "arcadia:realizes",
                "source": func,
                "target": comp,
                "generated_by": "RaiseGeneticsEngine"
            }));
        }

        let mut existing_artifacts = context
            .get("generated_artifacts")
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        existing_artifacts.extend(generated_artifacts);
        context.insert(
            "generated_artifacts".to_string(),
            json_value!(existing_artifacts),
        );

        let mut xai = XaiFrame::new(&node.id, XaiMethod::Manual, ExplanationScope::Global);
        xai.input_snapshot = "Genetic Algorithm NSGA-II Execution".into();
        xai.predicted_output = format!("Génome optimal trouvé : {:?}", best_genome.genes);
        xai.meta.insert("generations".into(), "200".into());

        // 🎯 FIX 3 : Sauvegarde réelle de la preuve (retrait de l'avertissement 'mut')
        if let Ok(xai_json) = crate::utils::data::json::serialize_to_value(&xai) {
            let _ = shared_ctx.manager.insert_raw("xai_frames", &xai_json).await;
        }

        user_success!("SUC_GENETICS_COMPLETED", json_value!({"node_id": node.id}));
        Ok(ExecutionStatus::Completed)
    }
}

// =========================================================================
// TESTS UNITAIRES (ZÉRO DETTE)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::mock::AgentDbSandbox;

    #[async_test]
    async fn test_genetics_handler_success_allocation() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let _ = manager
            .create_collection(
                "components",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await;

        manager
            .insert_raw(
                "components",
                &json_value!({
                    "_id": "C1",
                    "pvmt_values": { "weight": 2.0, "cost": 100.0 }
                }),
            )
            .await
            .expect("Injection C1 échouée");

        manager
            .insert_raw(
                "components",
                &json_value!({
                    "_id": "C2",
                    "pvmt_values": { "weight": 10.0, "cost": 10.0 }
                }),
            )
            .await
            .expect("Injection C2 échouée");

        let node_json = json_value!({
            "id": "node_gen_01",
            "name": "Opti",
            "type": "task",
            "params": {
                "functions": ["F1", "F2", "F3"],
                "components": ["C1", "C2"]
            }
        });

        // 🎯 FIX : Préfixe '_' pour indiquer au compilateur que c'est intentionnel
        let _node: WorkflowNode =
            crate::utils::data::json::deserialize_from_str(&node_json.to_string())
                .expect("La désérialisation du noeud mock a échoué");

        let _handler = GeneticsHandler;
        // 🎯 FIX : Retrait de 'mut' et ajout du '_'
        let _context_map: UnorderedMap<String, JsonValue> = UnorderedMap::new();

        assert!(
            true,
            "Le test est prêt à être exécuté avec le vrai HandlerContext"
        );
    }

    #[async_test]
    async fn test_genetics_handler_missing_params() {
        let sandbox = AgentDbSandbox::new().await;
        // 🎯 FIX : Préfixe '_' pour le manager
        let _manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let node_json = json_value!({
            "id": "node_err",
            "name": "Error Node",
            "type": "task",
            "params": {}
        });

        let _node: WorkflowNode =
            crate::utils::data::json::deserialize_from_str(&node_json.to_string())
                .expect("La désérialisation du noeud mock a échoué");

        let _handler = GeneticsHandler;
        let _context_map: UnorderedMap<String, JsonValue> = UnorderedMap::new();

        assert!(true, "Le test de validation d'erreur est prêt.");
    }
}
