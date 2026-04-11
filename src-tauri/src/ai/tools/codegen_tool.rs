// FICHIER : src-tauri/src/ai/tools/codegen_tool.rs

use crate::ai::protocols::mcp::{McpTool, McpToolCall, McpToolResult, ToolDefinition};
use crate::code_generator::analyzers::semantic_analyzer::SemanticAnalyzer;
use crate::code_generator::analyzers::Analyzer;
use crate::code_generator::models::{
    CodeElement, CodeElementType, Module, TargetLanguage, Visibility,
};
use crate::code_generator::CodeGeneratorService;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*; // 🎯 Façade Unique

pub struct CodeGenTool {
    service: CodeGeneratorService,
    db: SharedRef<StorageEngine>,
    space: String,
    db_name: String,
}

impl CodeGenTool {
    /// Initialise l'outil de génération de code.
    pub fn new(
        domain_root: PathBuf,
        db: SharedRef<StorageEngine>,
        space: &str,
        db_name: &str,
    ) -> Self {
        Self {
            service: CodeGeneratorService::new(domain_root),
            db,
            space: space.to_string(),
            db_name: db_name.to_string(),
        }
    }

    /// Récupère un composant Arcadia à travers les collections d'architecture.
    async fn fetch_component(&self, id: &str) -> RaiseResult<JsonValue> {
        let manager = CollectionsManager::new(&self.db, &self.space, &self.db_name);
        let collections = ["pa_components", "la_components", "sa_components"];

        for col in collections {
            match manager.get_document(col, id).await {
                Ok(Some(doc)) => return Ok(doc),
                Ok(None) => continue,
                Err(e) => {
                    user_error!(
                        "ERR_DB_READ_FAILED",
                        json_value!({ "col": col, "error": e.to_string() })
                    );
                }
            }
        }

        raise_error!(
            "ERR_DB_COMPONENT_NOT_FOUND",
            error = format!(
                "Composant ID '{}' introuvable dans les collections d'ingénierie.",
                id
            ),
            context = json_value!({ "id": id, "searched_collections": collections })
        )
    }

    /// Détermine le langage cible via les métadonnées d'implémentation.
    fn determine_language(&self, doc: &JsonValue) -> RaiseResult<TargetLanguage> {
        let tech = doc
            .get("implementation")
            .and_then(|i| i.get("technology"))
            .and_then(|t| t.as_str())
            .unwrap_or("unknown");

        match tech {
            "rust" => Ok(TargetLanguage::Rust),
            "cpp" => Ok(TargetLanguage::Cpp),
            "ts" => Ok(TargetLanguage::TypeScript),
            _ => raise_error!(
                "ERR_CODEGEN_UNSUPPORTED_TECH",
                error = format!(
                    "La technologie '{}' n'est pas supportée par le générateur.",
                    tech
                ),
                context = json_value!({ "tech": tech })
            ),
        }
    }
}

#[async_interface]
impl McpTool for CodeGenTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "generate_component_code".into(),
            description: "Synchronise le composant Arcadia avec le code source physique via analyse sémantique.".into(),
            input_schema: json_value!({
                "type": "object",
                "properties": { "component_id": { "type": "string" } },
                "required": ["component_id"]
            }),
        }
    }

    async fn execute(&self, call: McpToolCall) -> McpToolResult {
        let component_id = match call.arguments["component_id"].as_str() {
            Some(id) => id,
            None => {
                return McpToolResult::error(
                    call.id,
                    "ID de composant manquant dans les arguments.",
                )
            }
        };

        // 1. Récupération et détection du langage avec Match strict
        let doc = match self.fetch_component(component_id).await {
            Ok(d) => d,
            Err(e) => return McpToolResult::error(call.id, &e.to_string()),
        };

        let lang = match self.determine_language(&doc) {
            Ok(l) => l,
            Err(e) => return McpToolResult::error(call.id, &e.to_string()),
        };

        // 2. Initialisation du module de code
        let name = doc["name"].as_str().unwrap_or("component");
        let mut module = match Module::new(name, PathBuf::from(format!("{}.rs", name))) {
            Ok(m) => m,
            Err(e) => return McpToolResult::error(call.id, &e.to_string()),
        };

        // 3. Analyse sémantique et Tissage du code
        let analyzer = SemanticAnalyzer::new();
        let analysis = match analyzer.analyze(&doc) {
            Ok(a) => a,
            Err(e) => return McpToolResult::error(call.id, &e.to_string()),
        };

        module.elements.push(CodeElement {
            handle: format!("comp:{}", component_id),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            signature: format!("fn {}_logic()", name),
            body: Some(" { println!(\"RAISE Execution Logic\"); } ".to_string()),
            dependencies: analysis.dependencies,
            metadata: analysis.metadata,
            // 🎯 Nouveaux champs pour la topologie GNN/IA
            module_id: None,
            parent_id: None,
            attributes: vec![],
            docs: Some(format!(
                "Généré automatiquement pour le composant Arcadia : {}",
                name
            )),
            elements: vec![],
        });

        // 4. Persistance et formatage (Mount Point Resilience)
        match self.service.sync_module(module).await {
            Ok(path) => {
                if lang == TargetLanguage::Rust {
                    let _ = self.service.format_module(&path).await;
                }
                McpToolResult::success(call.id, json_value!({ "path": path.to_string_lossy() }))
            }
            Err(e) => McpToolResult::error(
                call.id,
                &format!("Échec de la synchronisation disque : {}", e),
            ),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES ET DE RÉSILIENCE
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::mock::AgentDbSandbox;

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_codegen_tool_execution_flow() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let comp_id = "test-comp-01";
        let generic_schema_uri = "db://_system/_system/schemas/v1/db/generic.schema.json";

        manager
            .create_collection("pa_components", generic_schema_uri)
            .await?;
        manager
            .upsert_document(
                "pa_components",
                json_value!({
                    "_id": comp_id,
                    "name": "EngineController",
                    "implementation": { "technology": "rust" }
                }),
            )
            .await?;

        // 🎯 RÉSILIENCE : On s'assure que le point de montage de génération existe
        let gen_path = sandbox.domain_root.join("src-gen");
        fs::ensure_dir_async(&gen_path).await?;

        let tool = CodeGenTool::new(
            gen_path,
            sandbox.db.clone(),
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let call = McpToolCall::new(
            "generate_component_code",
            json_value!({ "component_id": comp_id }),
        );
        let result = tool.execute(call).await;

        if result.is_error {
            panic!(
                "L'exécution de l'outil a échoué : {}",
                result.content["error"]
            );
        }

        assert!(result.content["path"]
            .as_str()
            .unwrap()
            .contains("EngineController.rs"));
        Ok(())
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_determine_language_logic() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let tool = CodeGenTool::new(PathBuf::from("/tmp"), sandbox.db.clone(), "test", "test");

        let doc_rust = json_value!({ "implementation": { "technology": "rust" } });
        let doc_cpp = json_value!({ "implementation": { "technology": "cpp" } });

        assert_eq!(tool.determine_language(&doc_rust)?, TargetLanguage::Rust);
        assert_eq!(tool.determine_language(&doc_cpp)?, TargetLanguage::Cpp);
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à une technologie non supportée
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_codegen_unsupported_technology() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let tool = CodeGenTool::new(PathBuf::from("/tmp"), sandbox.db.clone(), "test", "test");
        let doc_fortran = json_value!({ "implementation": { "technology": "fortran" } });

        let result = tool.determine_language(&doc_fortran);
        match result {
            Err(AppError::Structured(data)) => {
                assert_eq!(data.code, "ERR_CODEGEN_UNSUPPORTED_TECH")
            }
            _ => panic!("Le moteur aurait dû lever ERR_CODEGEN_UNSUPPORTED_TECH"),
        }
        Ok(())
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_fetch_component_error_handling() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let tool = CodeGenTool::new(
            sandbox.domain_root.clone(),
            sandbox.db.clone(),
            "void",
            "void",
        );

        let result = tool.fetch_component("ghost_id").await;
        assert!(result.is_err());
        match result {
            Err(AppError::Structured(data)) => assert_eq!(data.code, "ERR_DB_COMPONENT_NOT_FOUND"),
            _ => panic!("Type d'erreur incorrect pour composant manquant"),
        }
        Ok(())
    }
}
