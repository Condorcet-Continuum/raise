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
    pub fn new(db_manager: &'a CollectionsManager<'a>) -> Self {
        Self {
            db_manager,
            processor: JsonLdProcessor::new(),
        }
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
        let registry = VocabularyRegistry::global();
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
            .get_index_lock(&self.db_manager.space, &self.db_manager.db);
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
            .get_index_lock(&self.db_manager.space, &self.db_manager.db);
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
        if let Err(e) = self
            .processor
            .validate_required_fields(&node, &["@id", "@type"])
        {
            raise_error!(
                "ERR_SEMANTIC_VALIDATION_FAIL",
                error = "Le document ne respecte pas l'ontologie de base (manque @id ou @type)",
                context = json_value!({"details": e.to_string(), "collection": collection})
            );
        }

        self.processor.expand_in_place(&mut node);
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
                self.processor.compact_in_place(&mut d);
                Ok(Some(d))
            }
            Ok(None) => Ok(None),
            Err(e) => raise_error!(
                "ERR_DB_READ_FAIL",
                error = e,
                context = json_value!({"collection": collection, "id": id})
            ),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::DbSandbox;

    /// 💎 TEST 1 : Cycle de vie complet d'une ontologie in-index.
    #[async_test]
    #[serial_test::serial]
    async fn test_semantic_manager_ontology_lifecycle_in_index() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        VocabularyRegistry::init_mock_for_tests();

        let db_mgr = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.mount_points.simulation.domain,
            &sandbox.config.mount_points.simulation.db,
        );
        DbSandbox::mock_db(&db_mgr).await?;

        db_mgr
            .create_collection(
                "_ontologies",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        let semantic_mgr = SemanticManager::new(&db_mgr);
        let mock_ontology = json_value!({
            "@context": { "aero": "https://raise.io/aero#" },
            "@graph": [ { "@id": "aero:Spacecraft", "@type": "owl:Class" } ]
        });

        // 🎯 FIX DEADLOCK : Le test n'a plus besoin de verrouiller l'index avant l'appel !
        semantic_mgr
            .create_ontology("aero", "1.1", &mock_ontology)
            .await?;

        // 2. VÉRIFICATION PHYSIQUE
        let fetched = db_mgr.get_document("_ontologies", "ontology_aero").await?;
        assert!(
            fetched.is_some(),
            "L'ontologie n'est pas présente en base !"
        );

        // 3. VÉRIFICATION SÉMANTIQUE (Hot-Reload RCU)
        let registry = VocabularyRegistry::global();
        assert!(
            registry.has_class("https://raise.io/aero#Spacecraft"),
            "Le hot-reload en RAM a échoué."
        );

        Ok(())
    }

    /// 💎 TEST 2 : Validation sémantique et expansion in-place.
    #[async_test]
    #[serial_test::serial]
    async fn test_semantic_manager_insert_node_validation() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        VocabularyRegistry::init_mock_for_tests();

        let db_mgr = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        DbSandbox::mock_db(&db_mgr).await?;
        db_mgr
            .create_collection(
                "la_components",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        let semantic_mgr = SemanticManager::new(&db_mgr);

        let node = json_value!({
            "@id": "urn:uuid:456",
            "@type": ["la:LogicalComponent"],
            "name": "Component"
        });

        let saved = semantic_mgr
            .insert_semantic_node("la_components", node)
            .await?;

        let saved_type = saved
            .get("@type")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|val| val.as_str())
            .ok_or_else(|| build_error!("TEST_FAIL", error = "Expansion @type échouée"))?;

        assert_eq!(saved_type, "https://raise.io/la#LogicalComponent");

        Ok(())
    }
}
