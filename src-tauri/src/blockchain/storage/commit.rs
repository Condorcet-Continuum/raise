// src-tauri/src/blockchain/storage/commit.rs

use crate::blockchain::crypto::hashing::calculate_hash;
use crate::blockchain::crypto::signing::{verify_signature, KeyPair};
use crate::utils::{data::Value, prelude::*, DateTime};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum MutationOp {
    Create,
    Update,
    Delete,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Mutation {
    /// Identifiant unique de l'élément (URI JSON-LD).
    #[serde(rename = "@id")]
    pub element_id: String,
    /// Type d'opération (Create, Update, Delete).
    pub operation: MutationOp,
    /// Données de l'élément (Compatible ArcadiaElement).
    pub payload: Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArcadiaCommit {
    pub id: String,
    pub parent_hash: Option<String>,
    pub author: String,
    pub timestamp: DateTime<Utc>,
    pub mutations: Vec<Mutation>,
    pub merkle_root: String,
    pub signature: Vec<u8>,
}

impl ArcadiaCommit {
    /// Crée et signe un nouveau commit.
    pub fn new(
        mutations: Vec<Mutation>,
        parent_hash: Option<String>,
        merkle_root: String,
        keys: &KeyPair,
    ) -> Self {
        let mut commit = Self {
            id: String::new(),
            parent_hash,
            author: keys.public_key_hex(),
            timestamp: Utc::now(),
            mutations,
            merkle_root,
            signature: vec![],
        };

        let hash = commit.compute_content_hash();
        commit.id = hash.clone();
        commit.signature = keys.sign(&hash);
        commit
    }

    /// Génère le hash déterministe du contenu du commit pour la signature.
    pub fn compute_content_hash(&self) -> String {
        let content = json!({
            "parent_hash": self.parent_hash,
            "author": self.author,
            "timestamp": self.timestamp,
            "mutations": self.mutations,
            "merkle_root": self.merkle_root
        });
        calculate_hash(&content)
    }

    /// Vérifie la validité de la signature et de l'intégrité du contenu.
    pub fn verify(&self) -> bool {
        let content_hash = self.compute_content_hash();
        verify_signature(&self.author, &content_hash, &self.signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::crypto::signing::KeyPair;

    #[test]
    fn test_mutation_serialization_at_id() {
        let muta = Mutation {
            element_id: "urn:pa:test".to_string(),
            operation: MutationOp::Create,
            payload: json!({"name": "TestComponent"}),
        };

        let serialized = serde_json::to_string(&muta).unwrap();
        assert!(serialized.contains("\"@id\":\"urn:pa:test\""));
    }

    #[test]
    fn test_commit_creation_and_verification() {
        let keys = KeyPair::generate();
        let mutations = vec![Mutation {
            element_id: "node:1".to_string(),
            operation: MutationOp::Create,
            payload: json!({
                "@type": "LogicalComponent",
                "name": "NavigationSystem"
            }),
        }];

        let commit = ArcadiaCommit::new(mutations, None, "test_root".to_string(), &keys);

        assert!(!commit.id.is_empty());
        assert!(commit.verify(), "Le commit signé devrait être valide");
    }

    #[test]
    fn test_commit_integrity_failure() {
        let keys = KeyPair::generate();
        let mut commit = ArcadiaCommit::new(vec![], None, "root".to_string(), &keys);

        // Altération du contenu après signature
        commit.merkle_root = "corrupted_root".to_string();

        assert!(!commit.verify(), "Un commit altéré ne doit pas être validé");
    }
}
