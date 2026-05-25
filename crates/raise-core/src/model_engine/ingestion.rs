// FICHIER : src-tauri/src/model_engine/ingestion.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::model_engine::arcadia::element_kind::ArcadiaSemantics;
use crate::model_engine::capella::model_reader::CapellaReader;
use crate::model_engine::types::ProjectModel;
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

pub struct ModelIngestionService;

impl ModelIngestionService {
    /// Ingestion asynchrone d'un fichier Capella (.aird / .capella)
    /// Utilise les points de montage pour la persistance résiliente.
    pub async fn ingest_capella(
        path: PathBuf,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<usize> {
        user_info!(
            "INF_INGESTION_CAPELLA_START",
            json_value!({"path": path.to_string_lossy()})
        );

        // 1. Délégation du parsing XML au pool CPU (Zéro Dette)
        let parse_result = spawn_cpu_task(move || CapellaReader::read_model(&path)).await;

        let model = match parse_result {
            Ok(res) => match res {
                Ok(m) => m,
                Err(e) => raise_error!(
                    "ERR_INGESTION_CAPELLA_PARSE",
                    error = e.to_string(),
                    context = json_value!({"action": "parsing_xml"})
                ),
            },
            Err(e) => raise_error!(
                "ERR_INGESTION_CPU_PANIC",
                error = e.to_string(),
                context = json_value!({"action": "spawn_cpu_task"})
            ),
        };

        // 2. Persistance dans le Graphe de Données
        Self::persist_model(&model, manager).await
    }

    /// Hydratation du Knowledge Graph (JSON-DB) à partir d'un modèle en mémoire.
    /// Aligné sur les bonnes pratiques RAISE : Match...raise_error.
    pub async fn persist_model(
        model: &ProjectModel,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<usize> {
        let elements = model.all_elements();
        let count = elements.len();
        let config = AppConfig::get();

        for el in elements {
            // 1. Routage intelligent via la sémantique (ArcadiaSemantics)
            let category_str = format!("{:?}", el.get_category()).to_lowercase() + "s";
            let collection_name = if category_str == "others" {
                "elements".to_string()
            } else {
                category_str
            };

            // 2. Vérification/Création résiliente de la collection via schéma générique
            // Utilise le point de montage système pour la définition du schéma
            let schema_uri = format!(
                "db://{}/{}/schemas/v1/db/generic.schema.json",
                config.mount_points.system.domain, config.mount_points.system.db
            );

            match manager
                .create_collection(&collection_name, &schema_uri)
                .await
            {
                Ok(_) => (),
                Err(e) => raise_error!("ERR_INGESTION_COLLECTION_SETUP", error = e.to_string()),
            }

            // 3. Sérialisation vers l'ontologie JSON-LD
            let mut doc = match json::serialize_to_value(el) {
                Ok(v) => v,
                Err(e) => raise_error!(
                    "ERR_INGESTION_SERIALIZATION",
                    error = e.to_string(),
                    context = json_value!({"element_id": el.id})
                ),
            };

            // Alignement ID physique / ID logique
            if let Some(obj) = doc.as_object_mut() {
                obj.insert("_id".to_string(), json_value!(el.id.clone()));
            }

            // 4. Insertion brute dans le moteur NoSQL
            match manager.insert_raw(&collection_name, &doc).await {
                Ok(_) => (),
                Err(e) => raise_error!(
                    "ERR_INGESTION_DB_INSERT",
                    error = e.to_string(),
                    context = json_value!({"element_id": el.id, "collection": collection_name})
                ),
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
// TESTS UNITAIRES (Respect des tests existants & Résilience Mount Points)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType};
    use crate::utils::testing::mock::AgentDbSandbox;

    #[async_test]
    async fn test_persist_model_routes_to_correct_collections() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        // 🎯 RÉSILIENCE MOUNT POINTS : Utilisation dynamique de la config système
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
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

        let result = ModelIngestionService::persist_model(&model, &manager).await?;
        assert_eq!(result, 2);

        // Vérification du routage sémantique
        let comp_doc = manager.get_document("components", "comp_1").await.unwrap();
        assert!(
            comp_doc.is_some(),
            "Le composant doit être dans 'components'"
        );

        let func_doc = manager.get_document("functions", "func_1").await.unwrap();
        assert!(func_doc.is_some(), "La fonction doit être dans 'functions'");

        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à un point de montage système invalide
    #[async_test]
    async fn test_ingestion_mount_point_resilience() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        // Manager pointant sur une partition fantôme
        let manager = CollectionsManager::new(&sandbox.db, "ghost_partition", "void_db");

        let mut model = ProjectModel::default();
        model.add_element(
            "test",
            "elements",
            ArcadiaElement {
                id: "err_1".into(),
                name: NameType::String("Err".into()),
                kind: "test#Element".into(),
                properties: UnorderedMap::new(),
            },
        );

        let result = ModelIngestionService::persist_model(&model, &manager).await;

        // L'ingestion doit lever une erreur structurée plutôt que de paniquer
        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_INGESTION_COLLECTION_SETUP");
                Ok(())
            }
            _ => panic!("L'ingestion aurait dû lever ERR_INGESTION_COLLECTION_SETUP"),
        }
    }
}
