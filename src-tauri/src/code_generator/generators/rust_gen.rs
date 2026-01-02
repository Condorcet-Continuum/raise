use super::{GeneratedFile, LanguageGenerator};
use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;
use tera::{Context, Tera};

pub struct RustGenerator {
    tera: Tera,
}

impl Default for RustGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl RustGenerator {
    pub fn new() -> Self {
        let mut tera = Tera::default();

        // Template Rust robuste
        // Il génère une structure propre qui compile
        tera.add_raw_template(
            "actor_struct",
            r#"
// ---------------------------------------------------------
// GÉNÉRÉ PAR RAISE (Module: code_generator)
// Type: {{ type }}
// ID: {{ id }}
// ---------------------------------------------------------

use serde::{Deserialize, Serialize};

/// {{ description }}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {{ class_name }} {
    pub id: String,
    pub name: String,
    // Ajoutez vos champs d'état ici
}

impl {{ class_name }} {
    /// Constructeur par défaut
    pub fn new() -> Self {
        Self {
            id: "{{ id }}".to_string(),
            name: "{{ name }}".to_string(),
        }
    }

    /// Logique métier principale
    pub fn execute(&self) -> Result<(), String> {
        println!("🚀 [{{ name }}] Exécution...");

        // AI_INJECTION_POINT
        // TODO: L'intelligence artificielle injectera la logique ici.
        
        Ok(())
    }
}
"#,
        )
        .unwrap();

        Self { tera }
    }
}

impl LanguageGenerator for RustGenerator {
    fn generate(&self, element: &Value) -> Result<Vec<GeneratedFile>> {
        let mut context = Context::new();

        // Extraction sécurisée des champs JSON-LD
        let name = element
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("UnknownElement");
        let id = element.get("id").and_then(|v| v.as_str()).unwrap_or("0000");
        let desc = element
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("Aucune description disponible.");
        let type_uri = element
            .get("@type")
            .and_then(|v| v.as_str())
            .unwrap_or("UnknownType");

        // Nettoyage du nom pour en faire une classe valide (PascalCase)
        // ex: "Superviseur de Vol" -> "SuperviseurDeVol"
        let class_name = name
            .split_whitespace()
            .map(s_upper_first)
            .collect::<Vec<String>>()
            .join("");

        // Nettoyage des caractères spéciaux
        let class_name = class_name.replace("'", "").replace("-", "");

        context.insert("name", name);
        context.insert("class_name", &class_name);
        context.insert("id", id);
        context.insert("description", desc);
        context.insert("type", type_uri);

        // Rendu du template
        let content = self.tera.render("actor_struct", &context)?;

        // Fichier de sortie : SuperviseurDeVol.rs
        let filename = format!("{}.rs", class_name);

        Ok(vec![GeneratedFile {
            path: PathBuf::from(filename),
            content,
        }])
    }
}

/// Helper pour mettre la première lettre en majuscule (PascalCase simple)
fn s_upper_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
