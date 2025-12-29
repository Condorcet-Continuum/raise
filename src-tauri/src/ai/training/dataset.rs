use crate::json_db::storage::StorageEngine;
use serde::Serialize;
// Retrait de 'serde_json::json' (inutilisé)
use std::fs::File;
use std::io::Write; // On garde Write pour le code principal
use tauri::{command, State};

#[derive(Serialize)]
struct TrainingExample {
    text: String,
}

/// --- 1. LOGIQUE MÉTIER PURE ---
pub fn internal_export_process(output_path: &str) -> Result<String, String> {
    let examples = vec![
        (
            "Crée un composant logiciel.",
            "J'ai créé le composant logiciel 'New_Component' dans la couche Logique (LA)."
        ),
        (
            "C'est quoi GenAptitude ?",
            "GenAptitude est un outil d'ingénierie système assisté par IA, basé sur la méthode Arcadia."
        ),
        (
            "Vérifie la cohérence du modèle.",
            "Analyse en cours... J'ai détecté 2 incohérences : un Acteur sans allocation et un Flux orphelin."
        ),
        (
            "Ajoute une fonction au système.",
            "Fonction système 'SF_Function_1' ajoutée au paquet racine."
        ),
    ];

    let mut file =
        File::create(output_path).map_err(|e| format!("Erreur création fichier: {}", e))?;

    for (instruction, response) in examples {
        let formatted_text = format!("<s>[INST] {} [/INST] {} </s>", instruction, response);

        let example = TrainingExample {
            text: formatted_text,
        };

        let json_line = serde_json::to_string(&example).map_err(|e| e.to_string())?;

        writeln!(file, "{}", json_line).map_err(|e| e.to_string())?;
    }

    Ok(format!(
        "Dataset exporté avec succès vers : {}",
        output_path
    ))
}

/// --- 2. COMMANDE TAURI ---
#[command]
pub async fn ai_export_dataset(
    _storage: State<'_, StorageEngine>,
    output_path: String,
) -> Result<String, String> {
    internal_export_process(&output_path)
}

/// --- 3. TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read; // On importe Read ici car seul le test en a besoin
                       // Retrait de 'std::path::PathBuf' (inutilisé, car déduit par inférence)

    #[test]
    fn test_export_generates_valid_jsonl() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_dataset_genaptitude.jsonl");
        let path_str = file_path.to_str().unwrap();

        let result = internal_export_process(path_str);

        assert!(result.is_ok(), "L'export a échoué : {:?}", result.err());
        assert!(file_path.exists(), "Le fichier n'a pas été créé");

        let mut file = File::open(&file_path).expect("Impossible d'ouvrir le fichier généré");
        let mut content = String::new();
        file.read_to_string(&mut content).expect("Lecture échouée");

        assert!(
            content.contains("[INST]"),
            "Le format Mistral [INST] manque"
        );
        assert!(
            content.contains("GenAptitude"),
            "Les données spécifiques manquent"
        );

        let line_count = content.lines().count();
        assert_eq!(line_count, 4, "Le nombre de lignes JSONL est incorrect");

        let _ = std::fs::remove_file(file_path);
    }
}
