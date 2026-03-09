// FICHIER : src-tauri/src/code_generator/mod.rs

pub mod analyzers;
pub mod generators;
pub mod templates;

use crate::utils::prelude::*;

use self::analyzers::dependency_analyzer::DependencyAnalyzer;
use self::analyzers::injection_analyzer::InjectionAnalyzer;
use self::generators::{
    cpp_gen::CppGenerator, rust_gen::RustGenerator, typescript_gen::TypeScriptGenerator,
    verilog_gen::VerilogGenerator, vhdl_gen::VhdlGenerator, LanguageGenerator,
};
use self::templates::template_engine::TemplateEngine;

// ✅ IMPORTS V2 (Architecture 100% Utils)
use self::analyzers::Analyzer;

// AJOUT : Derive Serializable pour l'affichage JSON dans les outils
#[derive(Debug, Clone, Copy, PartialEq, Serializable, Deserializable)]
pub enum TargetLanguage {
    Rust,
    TypeScript,
    Python,
    Cpp,
    Verilog,
    Vhdl,
}

pub struct CodeGeneratorService {
    root_path: PathBuf,
    template_engine: TemplateEngine,
    dep_analyzer: DependencyAnalyzer,
}

impl CodeGeneratorService {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            root_path,
            template_engine: TemplateEngine::new(),
            dep_analyzer: DependencyAnalyzer::new(),
        }
    }

    pub async fn generate_for_element(
        &self,
        element: &JsonValue,
        lang: TargetLanguage,
    ) -> RaiseResult<Vec<PathBuf>> {
        // 1. Analyse des dépendances
        let _analysis = self.dep_analyzer.analyze(element)?;

        // 2. Sélection du générateur
        let generator: Box<dyn LanguageGenerator> = match lang {
            TargetLanguage::Rust => Box::new(RustGenerator::new()),
            TargetLanguage::Verilog => Box::new(VerilogGenerator::new()),
            TargetLanguage::Vhdl => Box::new(VhdlGenerator::new()),
            TargetLanguage::Cpp => Box::new(CppGenerator::new()),
            TargetLanguage::TypeScript => Box::new(TypeScriptGenerator::new()),
            TargetLanguage::Python => {
                // ✅ On utilise la macro pour garder 100% de cohérence
                raise_error!(
                    "ERR_GENERATOR_NOT_IMPLEMENTED",
                    error = "Le générateur Python n'est pas encore implémenté.",
                    context = json_value!({
                        "target_language": "Python",
                        "action": "init_language_generator"
                    })
                );
            }
        };

        // 3. Génération "brute" (en mémoire)
        let mut files = generator.generate(element, &self.template_engine)?;
        let mut generated_paths = Vec::new();

        let scope = fs::ProjectScope::new_sync(&self.root_path)?;

        // Variable pour repérer la racine du Crate (pour Clippy)
        let mut crate_root: Option<PathBuf> = None;

        for file in &mut files {
            let full_path = self.root_path.join(&file.path);

            // Détection de la racine du projet Rust
            if file.path.ends_with("Cargo.toml") {
                if let Some(parent) = full_path.parent() {
                    crate_root = Some(parent.to_path_buf());
                }
            }

            // 4. Préservation du code (Fichier existant -> Injections)
            if full_path.exists() {
                if let Ok(injections) = InjectionAnalyzer::extract_injections(&full_path).await {
                    for (key, user_code) in injections {
                        let marker = format!("AI_INJECTION_POINT: {}", key);
                        if file.content.contains(&marker) {
                            user_info!(
                                "MSG_REINJECTION_FOUND", // 🎯 Code unique pour l'événement
                                json_value!({
                                    "key": key,
                                    "size_bytes": user_code.len()
                                })
                            );
                            file.content = file
                                .content
                                .replace(&marker, &format!("{}\n{}", marker, user_code));
                        }
                    }
                }
            }

            // 5. Écriture finale
            scope
                .write_async(&file.path, file.content.as_bytes())
                .await?;
            generated_paths.push(full_path);
        }

        // 6. POST-PROCESS : CLIPPY (Uniquement pour Rust)
        if lang == TargetLanguage::Rust {
            if let Some(path) = crate_root {
                self.apply_clippy(&path);
            }
        }

        Ok(generated_paths)
    }

    /// Exécute `cargo clippy --fix` sur le dossier généré.
    fn apply_clippy(&self, crate_path: &Path) {
        user_info!(
            "MSG_CLIPPY_EXEC_START", // 🎯 Code unique de l'événement
            json_value!({
                "path": crate_path.to_string_lossy() // 🎯 Contexte typé
            })
        );
        let args = [
            "clippy",
            "--fix",
            "--allow-dirty",
            "--allow-staged",
            "--",
            "-A",
            "clippy::all",
            "-D",
            "warnings",
        ];

        match os::exec_command_sync("cargo", &args, Some(crate_path)) {
            Ok(_) => {
                user_info!("CLIPPY_CLEANUP_SUCCESS");
            }
            Err(e) => {
                user_warn!(
                    "CLIPPY_ANALYSIS_FAILED",
                    json_value!({
                        "path": crate_path.to_string_lossy(),
                        "error": e.to_string()
                    })
                );
            }
        }

        let _ = os::exec_command_sync("cargo", &["fmt"], Some(crate_path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[async_test]
    async fn test_integration_analyzers() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();

        // 1. Initialisation manuelle d'un moteur de template vide et propre
        let mut engine = TemplateEngine::new();
        engine
            .add_raw_template("rust/actor", "pub struct MyComponent;")
            .unwrap();

        // 2. Création du service en injectant notre moteur déjà configuré
        // Si votre structure le permet, sinon utilisez le champ direct comme avant
        let mut service = CodeGeneratorService::new(root.clone());
        service.template_engine = engine;

        // 3. Préparation du fichier existant (pour tester la fusion/merge)
        let existing_file = root.join("MyComponent.rs");
        // On met des balises minimales pour ne pas perturber le parser
        let user_code = "fn custom() { println!(\"Preserved!\"); }";
        fs::write_atomic_async(&existing_file, user_code.as_bytes())
            .await
            .unwrap();

        // 4. Exécution de la génération
        let element = json_value!({
            "name": "MyComponent",
            "id": "A1"
        });

        let result = service
            .generate_for_element(&element, TargetLanguage::Rust)
            .await;

        // Si ça échoue ici, on veut voir l'erreur exacte du Service
        let paths = result.expect("Le service a échoué lors de la génération");

        // 5. Vérification finale
        let new_content = fs::read_to_string_async(&paths[0]).await.unwrap();

        // On vérifie que notre template mocké a été utilisé
        assert!(new_content.contains("MyComponent"));
    }
}
