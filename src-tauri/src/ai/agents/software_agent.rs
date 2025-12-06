use crate::ai::agents::{Agent, EngineeringIntent};
use crate::ai::llm::client::{LlmBackend, LlmClient};
use crate::code_generator::{CodeGeneratorService, TargetLanguage};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::fs;
use std::path::PathBuf;

pub struct SoftwareAgent {
    llm: LlmClient,
    storage: StorageEngine,
    output_dir: PathBuf,
}

impl SoftwareAgent {
    pub fn new(llm: LlmClient, storage: StorageEngine, root_path: PathBuf) -> Self {
        Self {
            llm,
            storage,
            output_dir: root_path.join("gen_workspace"),
        }
    }

    /// Cherche un élément dans la DB de manière souple (ignorant casse et espaces)
    fn find_element_by_name(&self, name: &str) -> Result<serde_json::Value> {
        let mgr = CollectionsManager::new(&self.storage, "un2", "_system");

        // Normalisation de la requête : "ControleurDeVol" -> "controleurdevol"
        let search_normalized = name.to_lowercase().replace(" ", "").replace("_", "");

        // On cherche dans les acteurs en priorité
        let collections_to_scan = vec!["actors", "functions", "components", "activities"];

        for col in collections_to_scan {
            // On ignore si la collection n'existe pas encore
            if let Ok(docs) = mgr.list_all(col) {
                for doc in docs {
                    if let Some(n) = doc.get("name").and_then(|v| v.as_str()) {
                        // Normalisation de la donnée DB : "Controleur De Vol" -> "controleurdevol"
                        let db_normalized = n.to_lowercase().replace(" ", "").replace("_", "");

                        // Comparaison souple
                        if db_normalized == search_normalized
                            || db_normalized.contains(&search_normalized)
                        {
                            return Ok(doc);
                        }
                    }
                }
            }
        }

        Err(anyhow!(
            "Élément '{}' non trouvé en base (recherche normalisée : '{}').",
            name,
            search_normalized
        ))
    }
}

#[async_trait]
impl Agent for SoftwareAgent {
    async fn process(&self, intent: &EngineeringIntent) -> Result<Option<String>> {
        match intent {
            EngineeringIntent::GenerateCode {
                language,
                filename,
                context,
            } => {
                println!(
                    "💻 SoftwareAgent: Début de la génération hybride pour {}...",
                    filename
                );

                // 1. DÉTECTION
                // On retire l'extension pour trouver le nom probable (ex: "Superviseur.rs" -> "Superviseur")
                let target_name = filename.split('.').next().unwrap_or("Unknown");

                println!("🔍 Recherche de l'élément '{}' dans la DB...", target_name);
                let element_doc = self.find_element_by_name(target_name)
                    .context("Impossible de trouver l'élément source pour générer le code. Avez-vous créé l'acteur avant ?")?;

                // 2. GÉNÉRATION SYMBOLIQUE
                println!("🏗️ Appel du CodeGenerator (Templates)...");
                let lang_enum = match language.to_lowercase().as_str() {
                    "rust" => TargetLanguage::Rust,
                    "typescript" | "ts" => TargetLanguage::TypeScript, // Prévision
                    _ => {
                        return Err(anyhow!(
                            "Seul Rust est supporté pour le mode hybride pour l'instant."
                        ))
                    }
                };

                let generator = CodeGeneratorService::new(self.output_dir.clone());
                let generated_files = generator.generate_for_element(&element_doc, lang_enum)?;

                if generated_files.is_empty() {
                    return Err(anyhow!("Le générateur n'a produit aucun fichier."));
                }

                let main_file_path = &generated_files[0];

                // 3. INJECTION NEURONALE
                println!("🧠 Appel du LLM pour injection de logique...");

                let mut code_content = fs::read_to_string(main_file_path)?;
                let marker = "// AI_INJECTION_POINT";

                if code_content.contains(marker) {
                    let prompt = format!(
                        "Tu es un expert Rust. Voici un contexte métier : '{}'. \
                        Écris uniquement les lignes de code Rust (println, calculs, logic) pour implémenter ce contexte. \
                        Ne réécris PAS la fonction, juste le corps. Pas de markdown.", 
                        context
                    );

                    let logic_code = self
                        .llm
                        .ask(
                            LlmBackend::LocalLlama,
                            "Tu es un générateur de code concis.",
                            &prompt,
                        )
                        .await?;
                    let clean_logic = logic_code
                        .replace("```rust", "")
                        .replace("```", "")
                        .trim()
                        .to_string();

                    code_content = code_content.replace(marker, &clean_logic);
                    fs::write(main_file_path, code_content)?;
                }

                Ok(Some(format!(
                    "💾 Code Hybride Généré !\nFichier : `{}`\nBase : Template Rust\nLogique : Injectée par IA", 
                    main_file_path.display()
                )))
            }
            _ => Ok(None),
        }
    }
}
