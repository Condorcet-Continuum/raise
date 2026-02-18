// src-tauri/src/blockchain/sync/delta.rs

use crate::blockchain::storage::commit::Mutation;
use crate::utils::prelude::*;

/// Représente l'écart de données (diff) entre deux points de la chaîne.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ArcadiaDelta {
    /// Liste des mutations nécessaires pour passer de l'état A à l'état B.
    pub patch: Vec<Mutation>,
    /// Le hash de départ (base) pour lequel ce delta est valide.
    pub from_hash: Option<String>,
    /// Le hash cible obtenu après application du delta.
    pub to_hash: String,
}

impl ArcadiaDelta {
    /// Crée un nouvel objet de delta.
    pub fn new(from_hash: Option<String>, to_hash: String) -> Self {
        Self {
            patch: Vec::new(),
            from_hash,
            to_hash,
        }
    }

    /// Ajoute une mutation au delta.
    pub fn add_mutation(&mut self, mutation: Mutation) {
        self.patch.push(mutation);
    }

    /// Vérifie si le delta contient des mutations.
    pub fn is_empty(&self) -> bool {
        self.patch.is_empty()
    }

    /// Retourne le nombre de mutations contenues dans le patch.
    pub fn len(&self) -> usize {
        self.patch.len()
    }
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::storage::commit::MutationOp;
    use serde_json::json;

    #[test]
    fn test_delta_creation() {
        let mut delta = ArcadiaDelta::new(Some("hash_old".into()), "hash_new".into());

        let muta = Mutation {
            element_id: "urn:pa:comp1".into(),
            operation: MutationOp::Update,
            payload: json!({"status": "active"}),
        };

        delta.add_mutation(muta);

        assert_eq!(delta.len(), 1);

        // Vérification sécurisée du hash de départ sans déplacement
        assert_eq!(delta.from_hash.as_deref(), Some("hash_old"));
        assert_eq!(delta.to_hash, "hash_new");
        assert!(!delta.is_empty());
    }

    #[test]
    fn test_delta_default_state() {
        let delta = ArcadiaDelta::default();
        assert!(delta.is_empty());
        assert!(delta.from_hash.is_none());
        assert_eq!(delta.to_hash, "");
    }
}
