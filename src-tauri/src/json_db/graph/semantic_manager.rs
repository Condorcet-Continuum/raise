// FICHIER : src-tauri/src/json_db/graph/semantic_manager.rs

use crate::json_db::collections::manager::{CollectionsManager, EntityIdentity};
use crate::json_db::jsonld::{JsonLdProcessor, VocabularyRegistry};
use crate::utils::prelude::*;

/// Le `SemanticManager` est l'orchestrateur de haut niveau.
/// Il fait le pont entre la validation sémantique stricte (le Cerveau JSON-LD)
/// et le stockage physique structuré (le Muscle CollectionsManager).
pub struct SemanticManager<'a> {
    pub db_manager: &'a CollectionsManager<'a>,
    pub processor: JsonLdProcessor,
}

impl<'a> SemanticManager<'a> {
    /// Initialise le gestionnaire sémantique par-dessus un gestionnaire de collections existant.
    pub fn new(db_manager: &'a CollectionsManager<'a>) -> RaiseResult<Self> {
        Ok(Self {
            db_manager,
            processor: JsonLdProcessor::new()?,
        })
    }
    // =========================================================================
    // OPÉRATIONS DDL : GESTION DU MÉTA-MODÈLE (ONTOLOGIES)
    // =========================================================================

    /// Crée ou met à jour une ontologie (Opération DDL Sécurisée In-Index).
    pub async fn create_ontology(
        &self,
        namespace: &str,
        version: &str,
        content: &JsonValue,
    ) -> RaiseResult<()> {
        // 1. TENTATIVE de validation sémantique (Le Hot Reload RCU)
        let registry = VocabularyRegistry::global()?;
        if let Err(e) = registry.load_layer_from_json(namespace, content).await {
            raise_error!(
                "ERR_ONTOLOGY_LOAD_FAIL",
                error = e,
                context =
                    json_value!({"namespace": namespace, "action": "validation_before_insert"})
            );
        }

        // 2. Préparation du document pour la collection système
        let ontology_id = format!("ontology_{}", namespace);
        let mut ontology_doc = content.clone();
        if let Some(obj) = ontology_doc.as_object_mut() {
            obj.insert("_id".to_string(), json_value!(ontology_id.clone()));
        }

        // 3. Écriture ACID via le CollectionsManager (Il gère ses propres verrous)
        if let Err(e) = self
            .db_manager
            .insert_with_schema("_ontologies", ontology_doc)
            .await
        {
            raise_error!(
                "ERR_DB_INSERT_ONTOLOGY",
                error = e,
                context = json_value!({"namespace": namespace})
            );
        }

        // 4. Mise à jour du Registre des Métadonnées dans l'Index (Verrou Local)
        let lock = self
            .db_manager
            .storage
            .get_index_lock(&self.db_manager.space, &self.db_manager.db)?;
        let guard = lock.lock().await;
        let mut tx = self.db_manager.begin_system_tx(&guard).await?;

        let new_entry = json_value!({
            "uri": format!("db://{}/{}/_ontologies/{}.json", self.db_manager.space, self.db_manager.db, ontology_id),
            "version": version,
            "imports": []
        });

        if !tx.document["ontologies"].is_object() {
            tx.document["ontologies"] = json_value!({});
        }
        tx.document["ontologies"][namespace] = new_entry;

        tx.commit().await?;

        user_info!(
            "MSG_ONTOLOGY_CREATED",
            json_value!({"namespace": namespace, "version": version})
        );
        Ok(())
    }

    /// Supprime une ontologie du système (Opération DDL).
    pub async fn drop_ontology(&self, namespace: &str) -> RaiseResult<()> {
        let ontology_id = format!("ontology_{}", namespace);

        // 1. Suppression physique (Gère ses propres verrous via remove_item_from_index)
        if let Err(e) = self
            .db_manager
            .delete_identity("_ontologies", EntityIdentity::Id(ontology_id))
            .await
        {
            user_warn!(
                "WRN_ONTOLOGY_NOT_FOUND_IN_DB",
                json_value!({"namespace": namespace, "error": e.to_string()})
            );
        }

        // 2. Suppression dans le registre de l'Index (Verrou local)
        let lock = self
            .db_manager
            .storage
            .get_index_lock(&self.db_manager.space, &self.db_manager.db)?;
        let guard = lock.lock().await;
        let mut tx = self.db_manager.begin_system_tx(&guard).await?;

        if let Some(ontologies) = tx
            .document
            .get_mut("ontologies")
            .and_then(|o| o.as_object_mut())
        {
            ontologies.remove(namespace);
        }
        tx.commit().await?;

        user_info!(
            "MSG_ONTOLOGY_DROPPED",
            json_value!({"namespace": namespace})
        );
        Ok(())
    }

    // =========================================================================
    // OPÉRATIONS DML : MANIPULATION DU GRAPHE DE CONNAISSANCES
    // =========================================================================

    /// Insère un nœud sémantique après validation stricte (Opération DML).
    pub async fn insert_semantic_node(
        &self,
        collection: &str,
        mut node: JsonValue,
    ) -> RaiseResult<JsonValue> {
        // 1. Résolution automatique du contexte (Ma proposition précédente)
        // On détecte la couche via le nom de la collection (ex: pa_components -> pa)
        let layer_prefix = collection.split('_').next().unwrap_or("data");
        self.apply_mbse_context(&mut node, layer_prefix)?;

        // 🎯 2. Création du processeur SYNCHRONISÉ avec le contexte
        let processor = JsonLdProcessor::new()?.with_doc_context(&node)?;

        // 3. Contrat de base JSON-LD
        if let Err(e) = processor.validate_required_fields(&node, &["@id", "@type"]) {
            raise_error!(
                "ERR_SEMANTIC_VALIDATION_FAIL",
                error = e,
                context = json_value!({"collection": collection})
            );
        }

        // 4. Validation Ontologique Stricte
        // On étend en RAM uniquement pour vérifier que l'élément est légal
        if let Some(type_uri) = processor.get_primary_type(&node) {
            let expanded_type = processor.context_manager().expand_term(&type_uri);
            if !VocabularyRegistry::global()?.has_class(&expanded_type) {
                raise_error!(
                    "ERR_SEMANTIC_UNKNOWN_TYPE",
                    error = format!(
                        "Le type '{}' n'appartient pas à l'ontologie.",
                        expanded_type
                    ),
                    context = json_value!({"collection": collection, "provided_type": type_uri})
                );
            }
        }

        // 5. Persistance via le Muscle
        self.db_manager.insert_with_schema(collection, node).await
    }

    /// Récupère un nœud et le compacte en place pour une lecture plus humaine.
    pub async fn get_compacted_node(
        &self,
        collection: &str,
        id: &str,
    ) -> RaiseResult<Option<JsonValue>> {
        match self.db_manager.get_document(collection, id).await {
            Ok(Some(mut d)) => {
                // On synchronise le processeur avec le document lu pour réussir la compaction
                let processor = JsonLdProcessor::new()?.with_doc_context(&d)?;
                processor.compact_in_place(&mut d);
                Ok(Some(d))
            }
            Ok(None) => Ok(None),
            Err(e) => raise_error!("ERR_DB_READ_FAIL", error = e),
        }
    }

    /// Helper privé pour injecter les métadonnées MBSE sans effort pour l'IA.
    fn apply_mbse_context(&self, node: &mut JsonValue, layer: &str) -> RaiseResult<()> {
        if let Some(obj) = node.as_object_mut() {
            if !obj.contains_key("@context") {
                let registry = VocabularyRegistry::global()?;
                if let Some(ctx) = registry.get_context_for_layer(layer) {
                    obj.insert("@context".to_string(), ctx);
                }
            }
        }
        Ok(())
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{AgentDbSandbox, DbSandbox};
    const GENERIC_SCHEMA: &str = "db://_system/_system/schemas/v1/db/generic.schema.json";

    /// 🎯 HELPER : Charge les ontologies de base en RAM pour permettre la validation des nœuds.
    async fn bootstrap_test_ontologies(semantic_mgr: &SemanticManager<'_>) -> RaiseResult<()> {
        let _ = semantic_mgr
            .db_manager
            .create_collection("_ontologies", GENERIC_SCHEMA)
            .await;

        let pa_ontology = json_value!({
            "@context": { "pa": "https://raise.io/pa#", "owl": "http://www.w3.org/2002/07/owl#" },
            "@graph": [ { "@id": "pa:PhysicalComponent", "@type": "owl:Class" } ]
        });
        let la_ontology = json_value!({
            "@context": { "la": "https://raise.io/la#", "owl": "http://www.w3.org/2002/07/owl#" },
            "@graph": [
                { "@id": "la:LogicalComponent", "@type": "owl:Class" },
                { "@id": "la:LogicalFunction", "@type": "owl:Class" }
            ]
        });

        semantic_mgr
            .create_ontology("pa", "1.0", &pa_ontology)
            .await?;
        semantic_mgr
            .create_ontology("la", "1.0", &la_ontology)
            .await?;
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_semantic_manager_ontology_lifecycle_in_index() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        VocabularyRegistry::init_mock_for_tests();

        let db_mgr = CollectionsManager::new(&sandbox.storage, "sim", "db");
        DbSandbox::mock_db(&db_mgr).await?;
        db_mgr
            .create_collection("_ontologies", GENERIC_SCHEMA)
            .await?;

        let semantic_mgr = SemanticManager::new(&db_mgr)?;
        let mock_ontology = json_value!({
            "@context": { "aero": "https://raise.io/aero#", "owl": "http://www.w3.org/2002/07/owl#" },
            "@graph": [ { "@id": "aero:Spacecraft", "@type": "owl:Class" } ]
        });

        semantic_mgr
            .create_ontology("aero", "1.1", &mock_ontology)
            .await?;

        let fetched = db_mgr.get_document("_ontologies", "ontology_aero").await?;
        assert!(fetched.is_some());

        let registry = VocabularyRegistry::global()?;
        assert!(registry.has_class("https://raise.io/aero#Spacecraft"));
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_semantic_manager_insert_node_validation() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        VocabularyRegistry::init_mock_for_tests();

        let db_mgr = CollectionsManager::new(&sandbox.storage, "system", "db");
        DbSandbox::mock_db(&db_mgr).await?;
        db_mgr
            .create_collection("la_components", GENERIC_SCHEMA)
            .await?;

        let semantic_mgr = SemanticManager::new(&db_mgr)?;
        bootstrap_test_ontologies(&semantic_mgr).await?;

        let node = json_value!({
            "@id": "urn:uuid:456",
            "@type": "la:LogicalComponent",
            "name": "Component"
        });

        let saved = semantic_mgr
            .insert_semantic_node("la_components", node)
            .await?;
        assert_eq!(saved["@id"], "urn:uuid:456");
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_auto_alignment_physical_layer() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        VocabularyRegistry::init_mock_for_tests();

        let db_mgr = CollectionsManager::new(&sandbox.db, "simu", "db");
        DbSandbox::mock_db(&db_mgr).await?;
        db_mgr
            .create_collection("pa_components", GENERIC_SCHEMA)
            .await?;

        let semantic_mgr = SemanticManager::new(&db_mgr)?;
        bootstrap_test_ontologies(&semantic_mgr).await?; // 🎯 FIX

        let raw_node = json_value!({
            "@id": "sensor-789",
            "@type": "pa:PhysicalComponent",
            "name": "Lidar_Front"
        });

        let saved_node = semantic_mgr
            .insert_semantic_node("pa_components", raw_node)
            .await?;
        assert!(saved_node.get("@context").is_some());
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_semantic_reference_resolution() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let db_mgr = CollectionsManager::new(&sandbox.db, "domain", "db");
        DbSandbox::mock_db(&db_mgr).await?;

        db_mgr
            .create_collection("la_functions", GENERIC_SCHEMA)
            .await?;
        db_mgr
            .create_collection("la_components", GENERIC_SCHEMA)
            .await?;

        let semantic_mgr = SemanticManager::new(&db_mgr)?;
        bootstrap_test_ontologies(&semantic_mgr).await?; // 🎯 FIX

        db_mgr
            .insert_raw(
                "la_functions",
                &json_value!({
                    "_id": "uuid-logic-calc-001",
                    "handle": "calc_logic",
                    "@type": "la:LogicalFunction"
                }),
            )
            .await?;

        let component = json_value!({
            "@id": "processor-01",
            "@type": "la:LogicalComponent",
            "executes": "ref:la_functions:handle:calc_logic"
        });

        let result = semantic_mgr
            .insert_semantic_node("la_components", component)
            .await?;
        assert_eq!(result["executes"], "uuid-logic-calc-001");
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_semantic_validation_rejection() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let db_mgr = CollectionsManager::new(&sandbox.db, "test", "db");
        let semantic_mgr = SemanticManager::new(&db_mgr)?;

        let ghost_node = json_value!({ "name": "Anonymous" });
        let result = semantic_mgr
            .insert_semantic_node("generic", ghost_node)
            .await;

        assert!(result.is_err());
        match result {
            Err(AppError::Structured(err)) => assert_eq!(err.code, "ERR_SEMANTIC_VALIDATION_FAIL"),
            _ => panic!("Type d'erreur incorrect"),
        }
        Ok(())
    }
}
