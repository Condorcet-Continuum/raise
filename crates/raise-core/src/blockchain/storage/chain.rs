// src-tauri/src/blockchain/storage/chain.rs
//! Registre local (Ledger) Mentis : Assure le stockage et le chaînage cryptographique des commits.

use crate::blockchain::storage::commit::MentisCommit;
use crate::utils::prelude::*;

#[derive(Debug, Serializable, Deserializable, Default)]
pub struct Ledger {
    /// Stockage brut des commits indexés par leur ID.
    pub commits: UnorderedMap<String, MentisCommit>,
    /// Pointeur vers la tête de la chaîne (Head).
    pub last_commit_hash: Option<String>,
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.commits.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commits.is_empty()
    }

    /// Ajoute un commit au registre de manière sécurisée.
    pub fn append_commit(&mut self, commit: MentisCommit) -> RaiseResult<()> {
        // 1. Vérification cryptographique absolue (Intégrité & Signature)
        if !commit.verify() {
            raise_error!("ERR_MENTIS_INTEGRITY", error = "INVALID_SIGNATURE");
        }

        // 2. 🎯 FIX : Vérification de la continuité de la chaîne (Anti-Fork)
        if let Some(ref last_hash) = self.last_commit_hash {
            if commit.parent_hash.as_ref() != Some(last_hash) {
                raise_error!(
                    "ERR_MENTIS_CHAIN_BROKEN",
                    error = "Le parent_hash ne correspond pas à la tête du Ledger local.",
                    context = json_value!({
                        "expected_parent": last_hash,
                        "received_parent": commit.parent_hash
                    })
                );
            }
        } else if commit.parent_hash.is_some() {
            // Cas du Genesis Block : Si le ledger local est vide, le premier commit reçu
            // ne doit idéalement pas avoir de parent, ou alors c'est qu'il nous manque l'historique.
            raise_error!(
                "ERR_MENTIS_MISSING_HISTORY",
                error = "Le registre local est vide, impossible de raccrocher un bloc avec parent."
            );
        }

        // 3. Intégration
        let id = commit.id.clone();
        self.last_commit_hash = Some(id.clone());
        self.commits.insert(id, commit);

        Ok(())
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::crypto::signing::KeyPair;

    #[test]
    fn test_ledger_methods() {
        let l = Ledger::new();
        assert_eq!(l.len(), 0);
        assert!(l.is_empty());
    }

    #[test]
    fn test_ledger_chain_continuity() {
        let mut ledger = Ledger::new();
        let keys = KeyPair::generate();

        // 1. Genesis Commit (Valide)
        let c1 = MentisCommit::new(vec![], None, &keys);
        let id1 = c1.id.clone();
        assert!(ledger.append_commit(c1).is_ok());

        // 2. Commit suivant légitime (Valide)
        let c2 = MentisCommit::new(vec![], Some(id1.clone()), &keys);
        assert!(ledger.append_commit(c2).is_ok());

        // 3. Commit Forké (Doit être rejeté)
        let c3_fork = MentisCommit::new(vec![], Some("mauvais_parent".into()), &keys);
        let result = ledger.append_commit(c3_fork);
        assert!(
            result.is_err(),
            "Le Ledger doit rejeter un bloc qui brise la chaîne."
        );
    }
}
