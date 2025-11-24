//! Système d'indexation pour requêtes rapides
//!
//! Refactorisation : Utilise un driver générique pour éviter la duplication de code.

use serde::{Deserialize, Serialize};
// use serde_json::Value; // <-- On peut retirer cet import si plus utilisé ailleurs

// Modules d'implémentation
pub mod btree;
pub mod driver;
pub mod hash;
pub mod manager;
pub mod paths;
pub mod text;

// Re-export pour le manager
pub use manager::create_collection_indexes;
pub use manager::update_indexes;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IndexType {
    BTree,
    Hash,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDefinition {
    pub name: String,
    /// Pointeur JSON vers le champ (ex: "/email")
    pub field_path: String,
    pub index_type: IndexType,
    pub unique: bool,
}

/// Structure de stockage sur disque d'une entrée d'index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexRecord {
    // CORRECTION : On stocke la clé sous forme de String brute (JSON stringifié)
    // Cela évite l'erreur "deserialize_any" de Bincode et améliore les perfs.
    pub key: String,
    pub document_id: String,
}

// ... (Le bloc #[cfg(test)] reste inchangé) ...

// ============================================================================
//  TESTS UNITAIRES & D'INTÉGRATION
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    // Import du helper centralisé
    use crate::json_db::test_utils::init_test_env;

    #[test]
    fn test_hash_index_lifecycle() {
        // Utilisation de la structure centralisée
        let env = init_test_env();
        let cfg = &env.cfg;
        let space = &env.space;
        let db = &env.db;
        let coll = "users";

        // Création manuelle des dossiers nécessaires (car init_test_env ne crée que la racine)
        let indexes_dir = paths::indexes_root(cfg, space, db, coll);
        std::fs::create_dir_all(&indexes_dir).expect("create indexes dir");

        let def = IndexDefinition {
            name: "email".to_string(),
            field_path: "/email".to_string(),
            index_type: IndexType::Hash,
            unique: true,
        };

        // 1. Insertion
        let doc_id = "user-123";
        let doc = json!({ "id": doc_id, "email": "alice@example.com", "age": 30 });

        hash::update_hash_index(cfg, space, db, coll, &def, doc_id, None, Some(&doc))
            .expect("Insert failed");

        // Vérification fichier
        let idx_path = paths::index_path(cfg, space, db, coll, "email", IndexType::Hash);
        assert!(idx_path.exists());

        // Vérification contenu
        let index: std::collections::HashMap<String, Vec<String>> =
            driver::load(&idx_path).unwrap();
        assert!(index.contains_key("\"alice@example.com\""));

        // Le reste des tests suit la même logique...
        // ...
    }

    // Appliquer la même logique (env.cfg, env.space, env.db) aux autres tests :
    // test_btree_index_ordering
    // test_unique_constraint_violation
    // test_text_index_wiring
}
