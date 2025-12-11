use genaptitude_core_api::{AnalysisReport, AnalysisStatus, CognitiveBlock, CognitiveModel};
use std::mem;
use std::slice;

// =========================================================================
// 1. LOGIQUE MÉTIER (Votre intelligence artificielle / Algo)
// =========================================================================

struct ConsistencyChecker;

impl CognitiveBlock for ConsistencyChecker {
    fn id(&self) -> &str {
        "fr.genaptitude.blocks.consistency.basic"
    }

    fn execute(&self, model: &CognitiveModel) -> Result<AnalysisReport, String> {
        let mut messages = Vec::new();
        let mut warning_count = 0;

        // Règle 1 : Vérifier que le modèle a un ID
        if model.id.trim().is_empty() {
            messages.push("ERREUR CRITIQUE: Le modèle n'a pas d'identifiant.".to_string());
            warning_count += 1;
        }

        // Règle 2 : Vérifier les éléments (exemple simple)
        if model.elements.is_empty() {
            messages.push("AVERTISSEMENT: Le modèle est vide.".to_string());
            warning_count += 1;
        } else {
            for (key, elem) in &model.elements {
                if elem.name.trim().is_empty() {
                    messages.push(format!("ERREUR: L'élément '{}' n'a pas de nom.", key));
                    warning_count += 1;
                }
            }
        }

        // Conclusion
        let status = if warning_count > 0 {
            AnalysisStatus::Warning
        } else {
            messages.push("Succès: Modèle valide.".to_string());
            AnalysisStatus::Success
        };

        Ok(AnalysisReport {
            block_id: self.id().to_string(),
            status,
            messages,
            timestamp: 0, // Idéalement, passer le timestamp depuis l'hôte
        })
    }
}

// =========================================================================
// 2. INTERFACE SYSTÈME (Le "Pont" avec Tauri)
// =========================================================================

/// Fonction utilitaire pour allouer de la mémoire depuis l'hôte (Tauri)
#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf); // On empêche Rust de libérer la mémoire tout de suite
    ptr
}

/// Fonction principale appelée par Tauri
/// Reçoit un pointeur vers le JSON d'entrée, retourne un pointeur vers le JSON de sortie
///
/// # Safety
/// Cette fonction est `unsafe` car elle manipule des pointeurs bruts (`*mut u8`).
/// L'appelant (l'hôte Tauri) doit garantir que :
/// 1. `ptr` pointe vers une zone mémoire valide allouée par `alloc`.
/// 2. `len` correspond exactement à la taille des données écrites.
#[no_mangle]
pub unsafe extern "C" fn run_analysis(ptr: *mut u8, len: usize) -> u64 {
    // A. Récupération des données envoyées par Tauri
    let input_bytes = slice::from_raw_parts(ptr, len);
    let input_str = String::from_utf8_lossy(input_bytes);

    // B. Exécution de la logique
    let output_json = match serde_json::from_str::<CognitiveModel>(&input_str) {
        Ok(model) => {
            let checker = ConsistencyChecker;
            match checker.execute(&model) {
                Ok(report) => serde_json::to_string(&report).unwrap_or_default(),
                Err(e) => format!("{{\"error\": \"Echec analyse: {}\"}}", e),
            }
        }
        Err(e) => format!("{{\"error\": \"JSON Invalide reçu par le WASM: {}\"}}", e),
    };

    // C. Préparation de la réponse
    let result_bytes = output_json.into_bytes();
    let result_len = result_bytes.len();
    let result_ptr = result_bytes.as_ptr();

    // Important : On "oublie" le vecteur pour que la mémoire reste valide le temps que Tauri la lise
    mem::forget(result_bytes);

    // D. On retourne [Pointeur (32b) | Longueur (32b)] compactés en un u64
    ((result_ptr as u64) << 32) | (result_len as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use genaptitude_core_api::{AnalysisStatus, CognitiveModel, ModelElement};
    use std::collections::HashMap;

    // Helper pour créer un modèle rapidement
    fn create_model(element_name: &str) -> CognitiveModel {
        let mut elements = HashMap::new();
        elements.insert(
            "e1".to_string(),
            ModelElement {
                name: element_name.to_string(),
                kind: "Test".to_string(),
                properties: HashMap::new(),
            },
        );

        CognitiveModel {
            id: "test-model".to_string(),
            elements,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_valid_model() {
        let model = create_model("Mon Composant Valide");
        let checker = ConsistencyChecker;

        let report = checker.execute(&model).unwrap();

        // On s'attend à un succès
        match report.status {
            AnalysisStatus::Success => (),
            _ => panic!(
                "Le modèle valide a été marqué comme échoué : {:?}",
                report.messages
            ),
        }
    }

    #[test]
    fn test_empty_name_detection() {
        // On crée un modèle invalide (nom vide)
        let model = create_model("");
        let checker = ConsistencyChecker;

        let report = checker.execute(&model).unwrap();

        // On s'attend à un Warning
        match report.status {
            AnalysisStatus::Warning => {
                assert!(report.messages[0].contains("pas de nom"));
            }
            _ => panic!("Le modèle invalide n'a pas déclenché de Warning"),
        }
    }
}
