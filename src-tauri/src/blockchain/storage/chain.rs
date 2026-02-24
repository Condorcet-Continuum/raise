// src-tauri/src/blockchain/storage/chain.rs

use crate::blockchain::storage::commit::ArcadiaCommit;

use crate::utils::{prelude::*, HashMap};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Ledger {
    /// Index des commits par leur hash (ID).
    pub commits: HashMap<String, ArcadiaCommit>,
    /// Hash du dernier commit validé (la tête de la chaîne).
    pub last_commit_hash: Option<String>,
}

impl Ledger {
    /// Crée un nouveau registre vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute un commit à la chaîne locale après vérification de sa validité.
    pub fn append_commit(&mut self, commit: ArcadiaCommit) -> RaiseResult<()> {
        // 1. Vérification de la signature et de l'intégrité
        if !commit.verify() {
            return Err(AppError::from("Signature ou intégrité du commit invalide"));
        }

        // 2. Vérification du chaînage (continuité)
        if commit.parent_hash != self.last_commit_hash {
            return Err(AppError::Validation(format!(
                "Erreur de continuité : le parent attendu est {:?}, reçu {:?}",
                self.last_commit_hash, commit.parent_hash
            )));
        }

        // 3. Insertion dans le registre
        let commit_id = commit.id.clone();
        self.last_commit_hash = Some(commit_id.clone());
        self.commits.insert(commit_id, commit);

        Ok(())
    }

    /// Récupère un commit spécifique par son hash.
    pub fn get_commit(&self, hash: &str) -> Option<&ArcadiaCommit> {
        self.commits.get(hash)
    }

    /// Retourne le nombre total de commits dans le registre.
    pub fn len(&self) -> usize {
        self.commits.len()
    }

    /// Indique si le registre est vide (requis par Clippy quand len() est présent).
    pub fn is_empty(&self) -> bool {
        self.commits.is_empty()
    }
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::crypto::signing::KeyPair;
    use crate::blockchain::storage::commit::{Mutation, MutationOp};
    use chrono::Utc;
    use serde_json::json;

    fn create_mock_commit(keys: &KeyPair, parent: Option<String>) -> ArcadiaCommit {
        let mut commit = ArcadiaCommit {
            id: String::new(),
            parent_hash: parent,
            author: keys.public_key_hex(),
            timestamp: Utc::now(),
            mutations: vec![Mutation {
                element_id: "urn:test:1".to_string(),
                operation: MutationOp::Create,
                payload: json!({"type": "Test"}),
            }],
            merkle_root: "root".to_string(),
            signature: vec![],
        };
        let hash = commit.compute_content_hash();
        commit.id = hash.clone();
        commit.signature = keys.sign(&hash);
        commit
    }

    #[test]
    fn test_ledger_basics() {
        let mut ledger = Ledger::new();
        assert!(ledger.is_empty());

        let keys = KeyPair::generate();
        let c1 = create_mock_commit(&keys, None);
        ledger.append_commit(c1).unwrap();

        assert!(!ledger.is_empty());
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn test_ledger_append_valid_chain() {
        let mut ledger = Ledger::new();
        let keys = KeyPair::generate();

        let c1 = create_mock_commit(&keys, None);
        let c1_hash = c1.id.clone();
        ledger.append_commit(c1).unwrap();

        let c2 = create_mock_commit(&keys, Some(c1_hash));
        assert!(ledger.append_commit(c2).is_ok());
    }
}
