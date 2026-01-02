use serde_json::Value;
use tauri::AppHandle;

// Ici, nous simulons l'appel à votre module code_generator existant.
// Plus tard, nous importerons : use crate::code_generator::generator::generate;

#[tauri::command]
pub async fn generate_source_code(
    _app: AppHandle,
    language: String,
    model: Value,
) -> Result<String, String> {
    println!("⚡ Demande de génération de code : {}", language);

    // Simulation de la génération (en attendant de brancher le vrai module Tera)
    // C'est ici que nous connecterons votre module 'code_generator' réel.

    let model_name = model
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("ProjetInconnu");

    let generated_code = match language.as_str() {
        "rust" => format!(
            "// Généré par RAISE pour le projet {}\n\npub struct SystemRoot {{\n    pub name: String,\n    pub components: Vec<Component>,\n}}\n\nimpl SystemRoot {{\n    pub fn new() -> Self {{\n        println!(\"System initialized\");\n        Self {{ name: \"{}\".into(), components: vec![] }}\n    }}\n}}", 
            model_name, model_name
        ),
        "python" => format!(
            "# Généré par RAISE pour le projet {}\n\nclass SystemRoot:\n    def __init__(self):\n        self.name = \"{}\"\n        self.components = []\n        print(\"System initialized\")", 
            model_name, model_name
        ),
        _ => return Err(format!("Langage non supporté : {}", language)),
    };

    Ok(generated_code)
}
