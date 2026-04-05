// FICHIER : src-tauri/src/model_engine/ingestion.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::model_engine::arcadia::element_kind::ArcadiaSemantics;
use crate::model_engine::capella::model_reader::CapellaReader;
use crate::model_engine::types::ProjectModel;
// use crate::model_engine::sysml2::mapper::Sysml2ToArcadiaMapper; // À décommenter si applicable
use crate::utils::prelude::*;

pub struct ModelIngestionService;

impl ModelIngestionService {
    /// Ingestion asynchrone d'un fichier Capella (.aird / .capella)
    pub async fn ingest_capella(
        path: PathBuf,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<usize> {
        user_info!(
            "INF_INGESTION_CAPELLA_START",
            json_value!({"path": path.to_string_lossy()})
        );

        // 1. Délégation du parsing XML lourd au pool CPU (Zéro Dette)
        let parse_result = spawn_cpu_task(move || CapellaReader::read_model(&path)).await;

        let model = match parse_result {
            Ok(Ok(m)) => m,
            Ok(Err(e)) => raise_error!(
                "ERR_INGESTION_CAPELLA_PARSE",
                error = e,
                context = json_value!({"action": "parsing_xml"})
            ),
            Err(e) => raise_error!(
                "ERR_INGESTION_CPU_PANIC",
                error = e,
                context = json_value!({"action": "spawn_cpu_task"})
            ),
        };

        // 2. Persistance dans le Graphe de Données
        Self::persist_model(&model, manager).await
    }

    /// Hydratation du Knowledge Graph (JSON-DB) à partir d'un modèle en mémoire
    pub async fn persist_model(
        model: &ProjectModel,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<usize> {
        let elements = model.all_elements();
        let count = elements.len();

        for el in elements {
            // 1. Routage intelligent via la sémantique (ArcadiaSemantics)
            // Ex: "Component" -> "components", "Function" -> "functions"
            let category_str = format!("{:?}", el.get_category()).to_lowercase() + "s";
            let collection_name = if category_str == "others" {
                "elements".to_string()
            } else {
                category_str
            };

            // Optionnel : S'assurer que la collection existe (utile pour les tests)
            let _ = manager
                .create_collection(
                    &collection_name,
                    "db://_system/_system/schemas/v1/db/generic.schema.json",
                )
                .await;

            // 2. Sérialisation vers l'ontologie JSON-LD
            let mut doc = match crate::utils::data::json::serialize_to_value(el) {
                Ok(v) => v,
                Err(e) => raise_error!(
                    "ERR_INGESTION_SERIALIZATION",
                    error = e,
                    context = json_value!({"element_id": el.id, "kind": el.kind})
                ),
            };

            // On s'assure que l'ID physique est bien aligné avec l'ID logique
            if let Some(obj) = doc.as_object_mut() {
                obj.insert("_id".to_string(), json_value!(el.id.clone()));
            }

            // 3. Insertion brute dans le moteur NoSQL
            if let Err(e) = manager.insert_raw(&collection_name, &doc).await {
                return Err(crate::build_error!(
                    "ERR_INGESTION_DB_INSERT",
                    error = e,
                    context = json_value!({"element_id": el.id, "collection": collection_name})
                ));
            }
        }

        user_success!(
            "SUC_INGESTION_COMPLETED",
            json_value!({"element_count": count})
        );
        Ok(count)
    }
}

// =========================================================================
// TESTS UNITAIRES (ZÉRO DETTE)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType};
    use crate::utils::testing::mock::AgentDbSandbox;

    #[async_test]
    async fn test_persist_model_routes_to_correct_collections() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let mut model = ProjectModel::default();

        // Ajout d'un composant
        model.add_element(
            "pa",
            "components",
            ArcadiaElement {
                id: "comp_1".into(),
                name: NameType::String("Moteur".into()),
                kind: "https://raise.io/ontology/arcadia/pa#PhysicalComponent".into(),
                properties: UnorderedMap::new(),
            },
        );

        // Ajout d'une fonction
        model.add_element(
            "pa",
            "functions",
            ArcadiaElement {
                id: "func_1".into(),
                name: NameType::String("Propulser".into()),
                kind: "https://raise.io/ontology/arcadia/pa#PhysicalFunction".into(),
                properties: UnorderedMap::new(),
            },
        );

        let result = ModelIngestionService::persist_model(&model, &manager).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);

        // Vérification du routage sémantique
        let comp_doc = manager.get_document("components", "comp_1").await.unwrap();
        assert!(
            comp_doc.is_some(),
            "Le composant doit être dans 'components'"
        );

        let func_doc = manager.get_document("functions", "func_1").await.unwrap();
        assert!(func_doc.is_some(), "La fonction doit être dans 'functions'");
    }
}
