// FICHIER : src-tauri/src/ai/tools/codegen_tool.rs

use crate::ai::protocols::mcp::{McpTool, McpToolCall, McpToolResult, ToolDefinition};
use crate::code_generator::{CodeGeneratorService, TargetLanguage};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::utils::{async_trait, io::PathBuf, prelude::*, Arc};

/// Outil MCP qui fait le pont entre l'IA, la Base de Données et le Générateur de Code.
pub struct CodeGenTool {
    service: CodeGeneratorService,
    db: Arc<StorageEngine>,
    space: String,
    db_name: String,
}

impl CodeGenTool {
    /// Initialise l'outil avec tout le contexte nécessaire :
    /// - domain_root : Où écrire les fichiers générés
    /// - db : Le moteur de stockage pour lire le modèle
    /// - space/db_name : Les coordonnées de la base (ex: "mbse2"/"drones")
    pub fn new(domain_root: PathBuf, db: Arc<StorageEngine>, space: &str, db_name: &str) -> Self {
        Self {
            service: CodeGeneratorService::new(domain_root),
            db,
            space: space.to_string(),
            db_name: db_name.to_string(),
        }
    }

    /// Récupère le document complet depuis la base de données via son ID interne.
    /// Parcourt les collections probables car l'ID est unique globalement.
    async fn fetch_component(&self, id: &str) -> RaiseResult<Value> {
        let manager = CollectionsManager::new(&self.db, &self.space, &self.db_name);
        let collections = ["pa_components", "la_components", "sa_components"];

        for col in collections {
            match manager.get_document(col, id).await {
                Ok(Some(doc)) => return Ok(doc),
                Ok(None) => continue,
                Err(e) => eprintln!("⚠️ Erreur lecture collection {}: {}", col, e),
            }
        }

        // ✅ PLUS de Err(), PLUS de point-virgule nécessaire après la macro
        raise_error!(
            "ERR_DB_COMPONENT_NOT_FOUND",
            context = json!({
                "component_id": id,
                "searched_collections": collections,
                "space": self.space,
                "database": self.db_name,
                "action": "resolve_component",
                "hint": format!(
                    "Le composant '{}' est absent des collections ciblées.",
                    id
                )
            })
        )
    }

    /// Détermine le langage cible depuis le JSON du composant
    fn determine_language(&self, component: &Value) -> RaiseResult<TargetLanguage> {
        let tech = component
            .get("implementation")
            .and_then(|i| i.get("technology"))
            .and_then(|t| t.as_str())
            .unwrap_or("unknown");

        match tech {
            "Rust_Crate" | "rust" => Ok(TargetLanguage::Rust),
            "Cpp_Class" | "cpp" | "c++" => Ok(TargetLanguage::Cpp),
            "TypeScript_Module" | "typescript" | "ts" => Ok(TargetLanguage::TypeScript),
            "Python_Module" | "python" => Ok(TargetLanguage::Python),
            "Verilog_Module" | "verilog" => Ok(TargetLanguage::Verilog),
            "VHDL_Entity" | "vhdl" => Ok(TargetLanguage::Vhdl),
            _ => {
                // 🛠️ Alerte de support technologique
                raise_error!(
                    "ERR_CODEGEN_UNSUPPORTED_TECH",
                    context = json!({
                        "received_tech": tech,
                        "action": "resolve_target_language",
                        "supported_languages": [
                            "rust", "cpp", "typescript", "python", "verilog", "vhdl"
                        ],
                        "hint": "La technologie spécifiée dans le mandat n'est pas reconnue par le générateur. Vérifiez la casse ou ajoutez le support dans le LanguageResolver."
                    })
                )
            }
        }
    }
}

#[async_trait]
impl McpTool for CodeGenTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "generate_component_code".into(),
            description: "Génère le code source pour un composant stocké en base.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "component_id": { "type": "string" },
                    "dry_run": { "type": "boolean" }
                },
                "required": ["component_id"]
            }),
        }
    }

    async fn execute(&self, call: McpToolCall) -> McpToolResult {
        let component_id = match call.arguments["component_id"].as_str() {
            Some(id) => id,
            None => return McpToolResult::error(call.id, "component_id manquant"),
        };

        // 1. Récupération des données (DB)
        let component_doc = match self.fetch_component(component_id).await {
            Ok(doc) => doc,
            Err(e) => return McpToolResult::error(call.id, &format!("Erreur DB: {}", e)),
        };

        // 2. Détermination du langage
        let lang = match self.determine_language(&component_doc) {
            Ok(l) => l,
            Err(e) => return McpToolResult::error(call.id, &format!("Erreur Config: {}", e)),
        };

        // 3. Génération (Service)
        // Note: Le service gère l'écriture disque si dry_run est faux (comportement par défaut du service)
        match self
            .service
            .generate_for_element(&component_doc, lang)
            .await
        {
            Ok(paths) => {
                let file_list: Vec<String> = paths
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();

                let message = format!(
                    "Génération réussie pour '{}' ({}). {} fichiers écrits.",
                    component_doc["name"].as_str().unwrap_or("?"),
                    json!(lang).as_str().unwrap_or("Code"),
                    file_list.len()
                );

                McpToolResult::success(
                    call.id,
                    json!({
                        "message": message,
                        "files": file_list
                    }),
                )
            }
            Err(e) => McpToolResult::error(call.id, &format!("Erreur Génération: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::JsonDbConfig;
    use crate::utils::io::tempdir;

    /// Helper pour initialiser l'environnement de test
    fn setup_test_env() {
        // ✅ On initialise le mock de configuration pour éviter la panique
        crate::utils::config::test_mocks::inject_mock_config();
    }

    #[tokio::test]
    async fn test_codegen_tool_full_integration() {
        setup_test_env();

        let dir = tempdir().unwrap();
        let db_root = dir.path().join("db");
        let gen_root = dir.path().join("src-gen");

        let config = JsonDbConfig::new(db_root.clone());
        let storage = Arc::new(StorageEngine::new(config));

        // On initialise le manager et peuple la base...
        let manager = CollectionsManager::new(&storage, "test_space", "test_db");
        manager.init_db().await.unwrap();

        let comp_id = "comp-rust-01";
        manager
            .upsert_document(
                "pa_components",
                json!({
                    "id": comp_id,
                    "name": "MyRustComponent",
                    "implementation": { "technology": "rust" }
                }),
            )
            .await
            .unwrap();

        let tool = CodeGenTool::new(gen_root.clone(), storage.clone(), "test_space", "test_db");
        let call = McpToolCall::new(
            "generate_component_code",
            json!({ "component_id": comp_id }),
        );

        let result = tool.execute(call).await;

        assert!(
            !result.is_error,
            "L'outil a retourné une erreur: {:?}",
            result.content
        );
        assert!(result.content["files"]
            .as_array()
            .map_or(false, |a| !a.is_empty()));
    }

    #[tokio::test]
    async fn test_codegen_tool_not_found() {
        setup_test_env();

        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = Arc::new(StorageEngine::new(config));

        let tool = CodeGenTool::new(dir.path().into(), storage, "s", "d");
        let call = McpToolCall::new(
            "generate_component_code",
            json!({ "component_id": "unknown_id" }),
        );

        let result = tool.execute(call).await;

        assert!(result.is_error);

        // ✅ On vérifie le CODE d'erreur structuré au lieu du message traduit
        let error_msg = result.content["error"].as_str().unwrap();
        assert!(
            error_msg.contains("ERR_DB_COMPONENT_NOT_FOUND"),
            "Le code d'erreur devrait être ERR_DB_COMPONENT_NOT_FOUND. Reçu : {}",
            error_msg
        );
    }
}
