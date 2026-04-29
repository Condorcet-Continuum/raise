// src-tauri/src/blockchain/crypto/hashing.rs
//! Moteur de hachage Mentis : Canonisation stricte, déterminisme SHA-256 et Arbres de Merkle.

use crate::utils::prelude::*;

/// Calcule un hash SHA-256 déterministe pour n'importe quelle donnée JSON Mentis.
/// 🤖 IA NOTE: On utilise BTreeMap pour forcer le tri alphabétique récursif des clés.
pub fn calculate_hash(value: &JsonValue) -> String {
    // 1. Canonisation récursive : on neutralise l'ordre d'insertion des clés
    let canonical_json = sort_json_recursive(value);

    // 2. Sérialisation compacte (sans espaces inutiles)
    let payload = json::serialize_to_string(&canonical_json).unwrap_or_else(|_| "{}".to_string());

    if payload == "null" || payload.is_empty() {
        return "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into();
    }

    // 3. Hachage SHA-256
    let mut hasher = CryptoSha256::new();
    hasher.update(payload.as_bytes());
    let result = hasher.finalize();

    // 4. Conversion manuelle en Hex
    result
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

/// Trie récursivement les objets JSON.
/// Vital pour que {a:1, b:2} produise le même hash que {b:2, a:1}.
fn sort_json_recursive(v: &JsonValue) -> JsonValue {
    match v {
        JsonValue::Object(map) => {
            let mut sorted = OrderedMap::new();
            for (k, val) in map {
                sorted.insert(k.clone(), sort_json_recursive(val));
            }
            JsonValue::Object(sorted.into_iter().collect())
        }
        JsonValue::Array(arr) => JsonValue::Array(arr.iter().map(sort_json_recursive).collect()),
        _ => v.clone(),
    }
}

/// Calcule la véritable racine de Merkle pour un ensemble de hashes Mentis.
/// Contrairement à une simple concaténation, cette fonction opère par paires (Tree).
pub fn calculate_merkle_root(hashes: &[String]) -> String {
    if hashes.is_empty() {
        return String::new();
    }

    // On crée une copie mutable pour travailler dessus par niveaux
    let mut current_level = hashes.to_vec();

    // On boucle jusqu'à ce qu'il ne reste qu'un seul hash (la racine)
    while current_level.len() > 1 {
        let mut next_level = Vec::new();

        // On groupe les hashs par paires (chunks de 2)
        for chunk in current_level.chunks(2) {
            let combined = match chunk {
                [h1, h2] => format!("{}{}", h1, h2), // Paire complète
                [h1] => format!("{}{}", h1, h1),     // Nombre impair, on duplique le dernier
                _ => unreachable!(),
            };

            // On hache la paire combinée
            let mut hasher = CryptoSha256::new();
            hasher.update(combined.as_bytes());
            let result = hasher.finalize();

            let hex_result = result
                .iter()
                .map(|byte| format!("{:02x}", byte))
                .collect::<String>();

            next_level.push(hex_result);
        }

        // On remonte d'un niveau dans l'arbre
        current_level = next_level;
    }

    current_level[0].clone()
}

// --- TESTS UNITAIRES ROBUSTES ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_determinism_robustness() {
        // Objets sémantiquement identiques avec ordres différents
        let v1 = json_value!({ "z": 100, "a": 1, "m": { "inner": 2, "alpha": 0 } });
        let v2 = json_value!({ "a": 1, "m": { "alpha": 0, "inner": 2 }, "z": 100 });

        let h1 = calculate_hash(&v1);
        let h2 = calculate_hash(&v2);

        assert_eq!(
            h1, h2,
            "Le hachage doit ignorer l'ordre des clés (Canonisation BTreeMap)."
        );
    }

    #[test]
    fn test_hash_hex_standard_compliance() {
        let val = json_value!({ "mentis": "sovereign" });
        let h = calculate_hash(&val);

        // Un hash SHA-256 hex doit faire 64 caractères
        assert_eq!(h.len(), 64);
        assert!(
            h.chars().all(|c| c.is_ascii_hexdigit()),
            "Le hash doit être en hexadécimal pur."
        );
    }

    #[test]
    fn test_empty_json_hash() {
        let h_empty_str = calculate_hash(&JsonValue::String("".into()));
        let h_null = calculate_hash(&JsonValue::Null);

        assert!(!h_empty_str.is_empty());
        assert_eq!(
            h_null,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_true_merkle_tree_calculation() {
        let h1 = "a".repeat(64);
        let h2 = "b".repeat(64);
        let h3 = "c".repeat(64);
        let h4 = "d".repeat(64);

        // Arbre à 2 feuilles
        let root_2 = calculate_merkle_root(&[h1.clone(), h2.clone()]);
        assert_eq!(root_2.len(), 64);

        // Arbre à 3 feuilles (Doit dupliquer h3 pour équilibrer)
        let root_3 = calculate_merkle_root(&[h1.clone(), h2.clone(), h3.clone()]);
        assert_eq!(root_3.len(), 64);

        // Arbre à 4 feuilles
        let root_4 = calculate_merkle_root(&[h1, h2, h3, h4]);
        assert_eq!(root_4.len(), 64);

        assert_ne!(
            root_3, root_4,
            "Les racines de 3 et 4 feuilles doivent différer"
        );
    }
}
