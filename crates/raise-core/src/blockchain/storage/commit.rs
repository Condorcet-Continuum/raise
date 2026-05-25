// src-tauri/src/blockchain/storage/commit.rs
//! Unité de valeur Mentis : Mutations de connaissance souveraines, immuables et auditables.

use crate::blockchain::crypto::hashing::{calculate_hash, calculate_merkle_root};
use crate::blockchain::crypto::signing::{verify_signature, KeyPair};
use crate::utils::prelude::*;

/// Opérations atomiques autorisées sur l'espace cognitif Raise.
#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub enum MutationOp {
    Create,
    Update,
    Delete,
}

/// Une modification unitaire de donnée, formatée pour l'indexation JsonLD.
#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub struct Mutation {
    /// Identifiant unique (URN) de l'élément.
    #[serde(rename = "@id")]
    pub element_id: String,
    /// Type de mouvement transactionnel.
    pub operation: MutationOp,
    /// Le "Savoir" (JsonLD) associé à cette mutation.
    pub payload: JsonValue,
}

/// Le bloc de transaction Mentis : L'actif circulant fondamental du marketplace.
/// Un bloc est une preuve cryptographique de l'existence et de l'intégrité d'un savoir.
#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub struct MentisCommit {
    /// Identifiant unique du bloc (Hash SHA-256 déterministe du contenu).
    pub id: String,
    /// Référence au bloc précédent (Garantit la continuité du Ledger).
    pub parent_hash: Option<String>,
    /// Clé publique de l'auteur (Le vendeur souverain).
    pub author: String,
    /// Horodatage certifié de la création.
    pub timestamp: UtcTimestamp,
    /// Liste ordonnée des mutations de connaissance incluses.
    pub mutations: Vec<Mutation>,
    /// Racine de Merkle scellant l'intégrité de l'ensemble des mutations.
    pub merkle_root: String,
    /// Signature cryptographique de l'ID par la clé privée de l'auteur.
    pub signature: Vec<u8>,
}

impl MentisCommit {
    /// Crée, scelle et signe un nouveau bloc de connaissance Mentis.
    /// 🎯 ÉVOLUTION : La Merkle Root est calculée automatiquement pour éviter les erreurs.
    pub fn new(mutations: Vec<Mutation>, parent_hash: Option<String>, keys: &KeyPair) -> Self {
        // 1. On calcule d'abord la Merkle Root des mutations pour sceller la liste
        let mutation_hashes: Vec<String> = mutations
            .iter()
            .map(|m| calculate_hash(&json::json_value!(m)))
            .collect();
        let merkle_root = calculate_merkle_root(&mutation_hashes);

        let mut commit = Self {
            id: String::new(),
            parent_hash,
            author: keys.public_key_hex(),
            timestamp: UtcClock::now(),
            mutations,
            merkle_root,
            signature: vec![],
        };

        // 2. On scelle le contenu global en générant l'ID
        let hash = commit.compute_content_hash();
        commit.id = hash.clone();

        // 3. On signe l'ID (et seulement l'ID) pour lier l'auteur au contenu
        commit.signature = keys.sign(&hash);
        commit
    }

    /// Recalcule dynamiquement le hash à partir des données réelles présentes dans le struct.
    /// Vital pour détecter toute altération de champ après création.
    pub fn compute_content_hash(&self) -> String {
        let content = json::json_value!({
            "parent_hash": self.parent_hash,
            "author": self.author,
            "timestamp": self.timestamp,
            "mutations": self.mutations,
            "merkle_root": self.merkle_root
        });
        calculate_hash(&content)
    }

    /// Vérifie l'intégrité et l'authenticité absolue du bloc Mentis.
    /// Retourne true si et seulement si les données, l'ID et la Signature concordent.
    pub fn verify(&self) -> bool {
        // Étape 1 : Vérification de la structure Merkle interne (Anti-Mutation Injection)
        let mutation_hashes: Vec<String> = self
            .mutations
            .iter()
            .map(|m| calculate_hash(&json::json_value!(m)))
            .collect();
        let expected_merkle = calculate_merkle_root(&mutation_hashes);
        if self.merkle_root != expected_merkle {
            return false;
        }

        // Étape 2 : Vérification de l'ID (Anti-Tampering)
        let expected_id = self.compute_content_hash();
        if self.id != expected_id {
            return false;
        }

        // Étape 3 : Vérification de la Signature (Anti-Spoofing)
        verify_signature(&self.author, &self.id, &self.signature)
    }
}

// =========================================================================
// TESTS UNITAIRES ULTRA-ROBUSTES (Audit de Sécurité Mentis)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::crypto::signing::KeyPair;

    /// Test 1: Flux nominal (Création -> Vérification).
    #[test]
    fn test_mentis_commit_lifecycle_robust() {
        let keys = KeyPair::generate();
        let mutations = vec![Mutation {
            element_id: "urn:mentis:01".into(),
            operation: MutationOp::Create,
            payload: json::json_value!({"data": "verified"}),
        }];
        let commit = MentisCommit::new(mutations, None, &keys);
        assert!(
            commit.verify(),
            "Le bloc devrait être valide après sa création."
        );
    }

    /// Test 2: Détection de corruption de la Merkle Root.
    #[test]
    fn test_mentis_commit_tamper_merkle() {
        let keys = KeyPair::generate();
        let mut commit = MentisCommit::new(vec![], None, &keys);
        commit.merkle_root = "fake_root".into();
        assert!(
            !commit.verify(),
            "Un changement de Merkle Root doit être rejeté."
        );
    }

    /// Test 3: Corruption des données + Recalcul d'ID (ID Spoofing).
    /// Vérifie que la signature rejette un ID recalculé sur des données frauduleuses.
    #[test]
    fn test_mentis_commit_tamper_id_spoofing() {
        let keys = KeyPair::generate();
        let mut commit = MentisCommit::new(vec![], None, &keys);

        commit.merkle_root = "malicious".into();
        commit.id = commit.compute_content_hash();

        assert!(
            !commit.verify(),
            "La signature doit rejeter un ID recalculé frauduleusement."
        );
    }

    /// Test 4: Vol d'identité (Changement d'auteur).
    #[test]
    fn test_mentis_commit_tamper_author() {
        let keys = KeyPair::generate();
        let mut commit = MentisCommit::new(vec![], None, &keys);
        commit.author = "fake_pubkey".into();
        assert!(
            !commit.verify(),
            "Le changement d'auteur doit invalider la signature."
        );
    }

    /// Test 5: Injection de mutation post-signature.
    #[test]
    fn test_mentis_commit_injection() {
        let keys = KeyPair::generate();
        let mut commit = MentisCommit::new(vec![], None, &keys);
        commit.mutations.push(Mutation {
            element_id: "stolen:data".into(),
            operation: MutationOp::Delete,
            payload: json::json_value!({}),
        });
        assert!(
            !commit.verify(),
            "L'ajout de mutations invalide le hash et la merkle root."
        );
    }

    /// Test 6: Malleabilité temporelle (Replay protection).
    #[test]
    fn test_mentis_commit_timestamp_tamper() {
        let keys = KeyPair::generate();
        let mut commit = MentisCommit::new(vec![], None, &keys);
        commit.timestamp = UtcClock::now();
        assert!(
            !commit.verify(),
            "La modification de l'heure doit invalider le bloc."
        );
    }

    /// Test 7: Corruption de la Signature.
    #[test]
    fn test_mentis_commit_signature_corruption() {
        let keys = KeyPair::generate();
        let mut commit = MentisCommit::new(vec![], None, &keys);
        if !commit.signature.is_empty() {
            commit.signature[0] ^= 0xFF;
        }
        assert!(!commit.verify(), "Une signature altérée doit être rejetée.");
    }

    /// Test 8: Conformité JsonLD @id.
    #[test]
    fn test_mentis_jsonld_compliance() {
        let muta = Mutation {
            element_id: "urn:test".into(),
            operation: MutationOp::Create,
            payload: json::json_value!({}),
        };
        let s = json::serialize_to_string(&muta).unwrap();
        assert!(
            s.contains("@id"),
            "Doit utiliser @id pour la compatibilité sémantique."
        );
    }

    /// Test 9: Intégrité du parent.
    #[test]
    fn test_mentis_commit_parent_tamper() {
        let keys = KeyPair::generate();
        let mut commit = MentisCommit::new(vec![], Some("p1".into()), &keys);
        commit.parent_hash = Some("p2".into());
        assert!(
            !commit.verify(),
            "Le changement de parent doit être détecté."
        );
    }

    /// Test 10: Ordre des mutations.
    #[test]
    fn test_mentis_mutation_order_sensitivity() {
        let keys = KeyPair::generate();
        let m1 = Mutation {
            element_id: "1".into(),
            operation: MutationOp::Create,
            payload: json::json_value!({}),
        };
        let m2 = Mutation {
            element_id: "2".into(),
            operation: MutationOp::Create,
            payload: json::json_value!({}),
        };

        let c1 = MentisCommit::new(vec![m1.clone(), m2.clone()], None, &keys);
        let c2 = MentisCommit::new(vec![m2, m1], None, &keys);

        assert_ne!(
            c1.id, c2.id,
            "L'ordre des mutations doit modifier l'ID du bloc."
        );
    }

    /// Test 11: Corruption du payload interne.
    #[test]
    fn test_mentis_payload_tampering() {
        let keys = KeyPair::generate();
        let mut commit = MentisCommit::new(
            vec![Mutation {
                element_id: "1".into(),
                operation: MutationOp::Create,
                payload: json::json_value!({"val": 1}),
            }],
            None,
            &keys,
        );

        commit.mutations[0].payload = json::json_value!({"val": 2});
        assert!(
            !commit.verify(),
            "La modification d'une valeur interne doit être détectée."
        );
    }

    /// Test 12: Bloc vide mais signé.
    #[test]
    fn test_mentis_empty_commit_validity() {
        let keys = KeyPair::generate();
        let commit = MentisCommit::new(vec![], None, &keys);
        assert!(
            commit.verify(),
            "Un bloc sans mutation mais signé par l'auteur est valide."
        );
    }
}
