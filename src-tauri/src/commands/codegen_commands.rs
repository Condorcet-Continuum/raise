use crate::code_generator::{CodeGeneratorService, TargetLanguage};
use serde_json::Value;
use std::fs;
use tauri::{AppHandle, Manager};

/// Commande Tauri pour déclencher la génération de code.
///
/// # Arguments
/// * `language` - Le langage cible ("rust", "cpp", "verilog", "vhdl", "typescript").
/// * `model` - L'objet JSON représentant l'élément Arcadia (Component, Actor, etc.).
///
/// # Retourne
/// Une liste de chemins absolus vers les fichiers générés.
#[tauri::command]
pub async fn generate_source_code(
    app: AppHandle,
    language: String,
    model: Value,
) -> Result<Vec<String>, String> {
    println!(
        "🚀 [CodeGen] Demande reçue : {} pour l'élément {:?}",
        language,
        model.get("name")
    );

    // 1. Résolution du chemin de sortie
    // On utilise le dossier de données de l'application + /generated_code
    // Ex sur Linux: ~/.local/share/raise/generated_code/
    let app_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let output_dir = app_dir.join("generated_code");

    // Création du dossier si nécessaire
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir)
            .map_err(|e| format!("Impossible de créer le dossier de sortie: {}", e))?;
    }

    // 2. Mapping du langage (String -> Enum)
    let target_lang = match parse_language(&language) {
        Ok(lang) => lang,
        Err(e) => return Err(e),
    };

    // 3. Instanciation du service et exécution
    // Note: Idéalement, le service pourrait être géré par tauri::State pour éviter de recharger les templates à chaque fois
    let service = CodeGeneratorService::new(output_dir.clone());

    let generated_paths = service
        .generate_for_element(&model, target_lang)
        .map_err(|e| format!("Erreur lors de la génération : {}", e))?;

    // 4. Conversion des PathBuf en String pour le retour JS
    let paths_as_strings: Vec<String> = generated_paths
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();

    println!("✅ [CodeGen] Fichiers générés : {:?}", paths_as_strings);
    Ok(paths_as_strings)
}

/// Helper pour convertir la string d'entrée en enum TargetLanguage
fn parse_language(lang: &str) -> Result<TargetLanguage, String> {
    match lang.to_lowercase().as_str() {
        "rust" | "rs" => Ok(TargetLanguage::Rust),
        "cpp" | "c++" | "cxx" => Ok(TargetLanguage::Cpp),
        "verilog" | "v" => Ok(TargetLanguage::Verilog),
        "vhdl" | "vhd" => Ok(TargetLanguage::Vhdl),
        "typescript" | "ts" => Ok(TargetLanguage::TypeScript),
        "python" | "py" => Err("Le générateur Python n'est pas encore activé.".to_string()),
        _ => Err(format!("Langage non supporté : {}", lang)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_parsing() {
        assert_eq!(parse_language("Rust").unwrap(), TargetLanguage::Rust);
        assert_eq!(parse_language("c++").unwrap(), TargetLanguage::Cpp);
        assert_eq!(parse_language("Verilog").unwrap(), TargetLanguage::Verilog);
        assert_eq!(parse_language("ts").unwrap(), TargetLanguage::TypeScript);

        assert!(parse_language("python").is_err());
        assert!(parse_language("unknown").is_err());
    }
}
