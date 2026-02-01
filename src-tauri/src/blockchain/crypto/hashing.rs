use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Calcule le hash SHA-256 d'une valeur JSON de manière strictement déterministe.
/// Utilise un BTreeMap pour forcer le tri des clés, neutralisant ainsi
/// les réglages globaux de 'preserve_order' de serde_json.
pub fn calculate_hash(data: &Value) -> String {
    // On convertit la Value en une structure récursive où chaque Map est un BTreeMap (trié)
    let canonical_data = to_canonical_string(data);

    let mut hasher = Sha256::new();
    hasher.update(canonical_data.as_bytes());
    let result = hasher.finalize();

    format!("{:x}", result)
}

/// Fonction récursive qui transforme une Value en String avec clés triées
fn to_canonical_string(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            // On force le passage par un BTreeMap pour garantir l'ordre alphabétique des clés
            let sorted_map: BTreeMap<_, _> = map.iter().collect();
            let mut pieces = Vec::new();
            for (k, v) in sorted_map {
                pieces.push(format!("\"{}\":{}", k, to_canonical_string(v)));
            }
            format!("{{{}}}", pieces.join(","))
        }
        Value::Array(arr) => {
            let pieces: Vec<String> = arr.iter().map(to_canonical_string).collect();
            format!("[{}]", pieces.join(","))
        }
        // Pour les types simples, on utilise la sérialisation standard
        _ => serde_json::to_string(value).unwrap_or_else(|_| "null".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deterministic_hashing() {
        // Obj1 : id avant name
        let obj1 = json!({
            "id": "urn:pa:123",
            "name": "Ecu_Physical_Component",
            "type": "pa:PhysicalComponent"
        });

        // Obj2 : name avant id
        let obj2 = json!({
            "name": "Ecu_Physical_Component",
            "id": "urn:pa:123",
            "type": "pa:PhysicalComponent"
        });

        let hash1 = calculate_hash(&obj1);
        let hash2 = calculate_hash(&obj2);

        // Cette fois-ci, c'est mathématiquement garanti par BTreeMap
        assert_eq!(
            hash1, hash2,
            "Le hash doit être identique malgré l'ordre des clés"
        );
    }

    #[test]
    fn test_hash_change_on_data_modification() {
        let obj1 = json!({"status": "draft"});
        let obj2 = json!({"status": "validated"});
        assert_ne!(calculate_hash(&obj1), calculate_hash(&obj2));
    }
}
