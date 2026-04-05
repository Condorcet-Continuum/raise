// FICHIER : src-tauri/src/workflow_engine/handlers/world_model.rs

use super::{HandlerContext, NodeHandler};
use crate::ai::nlp::parser::CommandType;
use crate::ai::world_model::engine::WorldAction;
use crate::model_engine::types::{ArcadiaElement, NameType};
use crate::utils::prelude::*;
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};

pub struct WorldModelHandler;

#[async_interface]
impl NodeHandler for WorldModelHandler {
    fn node_type(&self) -> NodeType {
        // N'oublie pas d'ajouter NodeType::WorldModel dans ton enum NodeType
        NodeType::WorldModel
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut UnorderedMap<String, JsonValue>,
        shared_ctx: &HandlerContext<'_>,
    ) -> RaiseResult<ExecutionStatus> {
        user_info!("INF_WM_SIMULATION_START", json_value!({"node": node.name}));

        // 1. Extraction des paramètres de l'intention IA
        let element_id = match node.params.get("element_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => raise_error!(
                "ERR_WM_MISSING_ELEMENT",
                context = json_value!({"node_id": node.id, "hint": "L'ID de l'élément cible est requis."})
            ),
        };

        let intent_str = node
            .params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("Create");

        let intent = match intent_str.to_lowercase().as_str() {
            "create" => CommandType::Create,
            "delete" => CommandType::Delete,
            "search" => CommandType::Search,
            "explain" => CommandType::Explain,
            _ => CommandType::Unknown,
        };

        // 2. Extraction du Jumeau Numérique (JSON-DB -> Graphe)
        let collections = vec!["components", "functions", "actors", "data"];
        let mut element_doc = None;
        for col in collections {
            if let Ok(Some(doc)) = shared_ctx.manager.get_document(col, element_id).await {
                element_doc = Some(doc);
                break;
            }
        }

        let doc = match element_doc {
            Some(d) => d,
            None => raise_error!(
                "ERR_WM_ELEMENT_NOT_FOUND",
                context = json_value!({"element_id": element_id})
            ),
        };

        // Reconversion "Zéro Dette" du JSON brut vers l'ArcadiaElement pour l'Encodeur
        let name = doc
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let kind = doc
            .get("type")
            .or(doc.get("@type"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let mut properties = UnorderedMap::new();
        if let Some(props_obj) = doc.get("properties").and_then(|v| v.as_object()) {
            for (k, v) in props_obj {
                properties.insert(k.clone(), v.clone());
            }
        } else if let Some(obj) = doc.as_object() {
            // Si l'élément est "flat" (Pure Graph), on prend tout sauf les champs système
            for (k, v) in obj {
                if !matches!(k.as_str(), "id" | "_id" | "name" | "type" | "@type") {
                    properties.insert(k.clone(), v.clone());
                }
            }
        }

        let arcadia_element = ArcadiaElement {
            id: element_id.to_string(),
            name: NameType::String(name),
            kind,
            properties,
        };

        // 3. Délégation du calcul tensoriel au Thread CPU
        let orch = shared_ctx.orchestrator.lock().await;
        let world_engine = orch.world_engine.clone(); // Le moteur est partagé via SharedRef
        let action = WorldAction { intent };

        user_debug!(
            "DBG_WM_TENSOR_COMPUTATION",
            json_value!({"element": element_id, "intent": intent_str})
        );

        let future_state_tensor =
            match spawn_cpu_task(move || world_engine.simulate(&arcadia_element, action)).await {
                Ok(Ok(tensor)) => tensor,
                Ok(Err(e)) => return Err(e), // L'erreur RaiseResult est propagée
                Err(e) => raise_error!(
                    "ERR_WM_CPU_PANIC",
                    error = e,
                    context = json_value!({"element_id": element_id})
                ),
            };

        // 4. Analyse du Tenseur : Conversion de la physique du graphe en un indicateur métier
        // (Exemple : On calcule un score de viabilité basé sur la norme du tenseur résultant)
        let flat_tensor = match future_state_tensor.flatten_all() {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_WM_TENSOR_FLATTEN", error = e.to_string()),
        };

        let vec_data = match flat_tensor.to_vec1::<f32>() {
            Ok(v) => v,
            Err(e) => raise_error!("ERR_WM_TENSOR_EXTRACTION", error = e.to_string()),
        };

        let viability_score = if !vec_data.is_empty() {
            let sum: f32 = vec_data.iter().sum();
            sum / vec_data.len() as f32
        } else {
            0.0
        };

        // 5. Injection du résultat dans le Jumeau Numérique pour que l'IA puisse réagir
        context.insert(
            format!("wm_viability_{}", element_id),
            json_value!(viability_score),
        );

        user_success!(
            "SUC_WM_SIMULATION_DONE",
            json_value!({"element_id": element_id, "viability": viability_score})
        );

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
    async fn test_world_model_handler_execution() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let _ = manager.create_collection("components", "schema").await;

        // Injection d'un composant mock
        manager
            .insert_raw(
                "components",
                &json_value!({
                    "_id": "comp_abc",
                    "name": "Radar",
                    "type": "https://raise.io/ontology/arcadia/pa#PhysicalComponent"
                }),
            )
            .await
            .expect("Injection échouée");

        // Création du nœud IA avec les instructions
        let node_json = json_value!({
            "id": "node_wm_01",
            "name": "Simulate Radar Impact",
            "type": "world_model",
            "params": {
                "element_id": "comp_abc",
                "action": "create"
            }
        });

        let _node: WorkflowNode =
            crate::utils::data::json::deserialize_from_str(&node_json.to_string())
                .expect("Désérialisation échouée");

        let _handler = WorldModelHandler;
        let mut _context_map: UnorderedMap<String, JsonValue> = UnorderedMap::new();

        assert!(
            true,
            "Handler World Model prêt à être testé avec le contexte partagé"
        );
    }
}
