// FICHIER : src-tauri/src/json_db/graph/semantic_manager.rs

use crate::json_db::collections::manager::{CollectionsManager, SystemIndexTx};
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

    /// Crée ou met à jour une ontologie (Opération DDL).
    /// Enregistre le fichier, met à jour l'index système, et déclenche un Hot Reload.
    pub async fn create_ontology(
        &self,
        tx: &mut SystemIndexTx<'_>, // 🎯 FIX : Exigence du Jeton
        namespace: &str,
        version: &str,
        content: &JsonValue,
    ) -> RaiseResult<()> {
        let db_root = self
            .db_manager
            .storage
            .config
            .db_root(&self.db_manager.space, &self.db_manager.db);

        // 1. Écriture physique du fichier
        let ontology_dir = db_root.join("ontology");
        fs::ensure_dir_async(&ontology_dir).await?;
        let file_path = ontology_dir.join(format!("{}.jsonld", namespace));
        fs::write_json_atomic_async(&file_path, content).await?;

        // 2. Mise à jour du Manifeste (Index) - DIRECTEMENT DANS LE JETON
        let new_entry = json_value!({
            "uri": format!("db://{}/{}/ontology/{}.jsonld", self.db_manager.space, self.db_manager.db, namespace),
            "version": version,
            "imports": []
        });

        if !tx.document["ontologies"].is_object() {
            tx.document["ontologies"] = json_value!({});
        }
        tx.document["ontologies"][namespace] = new_entry;

        // 3. Hot Reload du Cerveau Sémantique Global
        let registry = VocabularyRegistry::global();
        registry.load_layer_from_file(namespace, &file_path).await?;

        user_info!(
            "MSG_ONTOLOGY_CREATED",
            json_value!({"namespace": namespace, "version": version})
        );
        Ok(())
    }

    /// Supprime une ontologie du système (Opération DDL).
    pub async fn drop_ontology(
        &self,
        tx: &mut SystemIndexTx<'_>,
        namespace: &str,
    ) -> RaiseResult<()> {
        let db_root = self
            .db_manager
            .storage
            .config
            .db_root(&self.db_manager.space, &self.db_manager.db);

        // 1. Suppression dans l'Index - DIRECTEMENT DANS LE JETON
        if let Some(ontologies) = tx
            .document
            .get_mut("ontologies")
            .and_then(|o| o.as_object_mut())
        {
            ontologies.remove(namespace);
        }

        // 2. Suppression physique
        let file_path = db_root.join(format!("ontology/{}.jsonld", namespace));
        if fs::exists_async(&file_path).await {
            fs::remove_file_async(&file_path).await?;
        }

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
        node: JsonValue,
    ) -> RaiseResult<JsonValue> {
        // 1. Le Cerveau valide la présence des ancrages vitaux
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

        // 2. Le Cerveau normalise et étend les URIs (Expansion)
        let expanded_node = self.processor.expand(&node);

        // 3. Le Muscle écrit sur le disque et valide le Schéma JSON (La syntaxe)
        self.db_manager
            .insert_with_schema(collection, expanded_node)
            .await
    }

    /// Récupère un nœud et le compacte pour une lecture plus humaine.
    pub async fn get_compacted_node(
        &self,
        collection: &str,
        id: &str,
    ) -> RaiseResult<Option<JsonValue>> {
        let doc = self.db_manager.get_document(collection, id).await?;
        match doc {
            Some(d) => Ok(Some(self.processor.compact(&d))),
            None => Ok(None),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Validation du couplage "Zéro Dette")
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::DbSandbox;

    #[async_test]
    #[serial_test::serial]
    async fn test_semantic_manager_ontology_lifecycle() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        VocabularyRegistry::init_mock_for_tests();

        let db_mgr = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.mount_points.simulation.domain,
            &sandbox.config.mount_points.simulation.db,
        );
        DbSandbox::mock_db(&db_mgr).await?;

        let semantic_mgr = SemanticManager::new(&db_mgr);

        let mock_ontology = json_value!({
            "@context": {
                "@version": 1.1,
                "aero": "https://raise.io/ontology/aerospace#"
            },
            "@graph": [
                {
                    "@id": "aero:Spacecraft",
                    "@type": "owl:Class"
                }
            ]
        });

        // 1. TEST : Création d'une ontologie métier (Avec Jeton)
        {
            let lock = db_mgr.storage.get_index_lock(&db_mgr.space, &db_mgr.db);
            let guard = lock.lock().await;
            let mut tx = db_mgr.begin_system_tx(&guard).await?;

            semantic_mgr
                .create_ontology(&mut tx, "aerospace", "1.1", &mock_ontology)
                .await?;
            tx.commit().await?;
        }

        // Vérification de l'écriture physique
        let db_root = sandbox.storage.config.db_root(&db_mgr.space, &db_mgr.db);
        let file_path = db_root.join("ontology/aerospace.jsonld");
        assert!(
            fs::exists_async(&file_path).await,
            "Le fichier JSON-LD doit exister sur le disque"
        );

        // Vérification du Hot Reload dans le registre
        let registry = VocabularyRegistry::global();
        assert!(
            registry.has_class("https://raise.io/ontology/aerospace#Spacecraft"),
            "L'ontologie n'a pas été chargée en RAM !"
        );

        // 2. TEST : Suppression de l'ontologie (Avec un nouveau Jeton)
        {
            let lock = db_mgr.storage.get_index_lock(&db_mgr.space, &db_mgr.db);
            let guard = lock.lock().await;
            let mut tx = db_mgr.begin_system_tx(&guard).await?;

            semantic_mgr.drop_ontology(&mut tx, "aerospace").await?;
            tx.commit().await?;
        }

        assert!(
            !fs::exists_async(&file_path).await,
            "Le fichier JSON-LD doit être supprimé"
        );

        Ok(())
    }

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

        // Création de la collection physique
        db_mgr
            .create_collection(
                "la_components",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        let semantic_mgr = SemanticManager::new(&db_mgr);

        // 1. TEST : Rejet d'un nœud non sémantique (Pas de @type)
        let invalid_node = json_value!({
            "@id": "urn:uuid:123",
            "name": "Invalid Component"
        });

        let err = semantic_mgr
            .insert_semantic_node("la_components", invalid_node)
            .await;
        assert!(
            err.is_err(),
            "L'insertion d'un nœud sans @type doit échouer"
        );
        assert!(err
            .unwrap_err()
            .to_string()
            .contains("ERR_SEMANTIC_VALIDATION_FAIL"));

        // 2. TEST : Insertion et Expansion d'un nœud valide
        let valid_node = json_value!({
            "@id": "urn:uuid:456",
            "@type": ["la:LogicalComponent"],
            "name": "Valid Component"
        });

        let saved = semantic_mgr
            .insert_semantic_node("la_components", valid_node)
            .await?;

        // Vérification que le type a bien été étendu (Expanded) avant la sauvegarde sur disque
        let saved_type = saved["@type"].as_array().unwrap()[0].as_str().unwrap();
        assert_eq!(
            saved_type,
            "https://raise.io/ontology/arcadia/la#LogicalComponent"
        );

        Ok(())
    }
}
