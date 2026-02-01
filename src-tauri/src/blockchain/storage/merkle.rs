// src-tauri/src/blockchain/storage/merkle.rs

use crate::blockchain::crypto::hashing::calculate_hash;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MerkleTree {
    pub root_hash: String,
    pub leaf_hashes: Vec<String>,
}

impl MerkleTree {
    /// Crée un nouvel arbre de Merkle à partir d'une liste de hashs de feuilles.
    /// Si la liste est vide, retourne un hash par défaut.
    pub fn new(leaves: Vec<String>) -> Self {
        if leaves.is_empty() {
            return Self {
                root_hash: calculate_hash(&json!("empty_tree")),
                leaf_hashes: leaves,
            };
        }

        let root_hash = Self::calculate_root(&leaves);
        Self {
            root_hash,
            leaf_hashes: leaves,
        }
    }

    /// Calcule récursivement la racine de l'arbre par paires.
    fn calculate_root(hashes: &[String]) -> String {
        if hashes.len() == 1 {
            return hashes[0].clone();
        }

        let mut next_level = Vec::new();
        for chunk in hashes.chunks(2) {
            match chunk {
                [h1, h2] => {
                    // On combine les deux hashs
                    let combined = format!("{}{}", h1, h2);
                    next_level.push(calculate_hash(&json!(combined)));
                }
                [h1] => {
                    // Si le nombre de feuilles est impair, on duplique le dernier pour équilibrer
                    let combined = format!("{}{}", h1, h1);
                    next_level.push(calculate_hash(&json!(combined)));
                }
                _ => unreachable!(),
            }
        }

        Self::calculate_root(&next_level)
    }
}

/// Implémentation de Default demandée par Clippy pour l'initialisation à vide.
impl Default for MerkleTree {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_root_determinism() {
        let leaves = vec![
            "hash1".to_string(),
            "hash2".to_string(),
            "hash3".to_string(),
        ];

        let tree1 = MerkleTree::new(leaves.clone());
        let tree2 = MerkleTree::new(leaves);

        assert_eq!(tree1.root_hash, tree2.root_hash);
    }

    #[test]
    fn test_merkle_empty() {
        let tree = MerkleTree::default();
        assert!(!tree.root_hash.is_empty());
        assert!(tree.leaf_hashes.is_empty());
    }

    #[test]
    fn test_merkle_consistency_with_different_order() {
        let leaves1 = vec!["a".to_string(), "b".to_string()];
        let leaves2 = vec!["b".to_string(), "a".to_string()];

        let tree1 = MerkleTree::new(leaves1);
        let tree2 = MerkleTree::new(leaves2);

        // L'ordre des feuilles doit changer la racine pour garantir l'intégrité de la séquence
        assert_ne!(tree1.root_hash, tree2.root_hash);
    }
}
