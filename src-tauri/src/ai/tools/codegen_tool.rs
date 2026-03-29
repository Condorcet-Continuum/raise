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
use crate::utils::prelude::*;

pub struct CodeGenTool {
    service: CodeGeneratorService,
    db: SharedRef<StorageEngine>,
    space: String,
    db_name: String,
}

impl CodeGenTool {
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

    async fn fetch_component(&self, id: &str) -> RaiseResult<JsonValue> {
        let manager = CollectionsManager::new(&self.db, &self.space, &self.db_name);
        let collections = ["pa_components", "la_components", "sa_components"];

        for col in collections {
            match manager.get_document(col, id).await {
                Ok(Some(doc)) => return Ok(doc),
                Ok(None) => continue,
                Err(e) => {
                    user_error!(
                        "ERR_DB_READ",
                        json_value!({ "col": col, "error": e.to_string() })
                    );
                }
            }
        }

        raise_error!(
            "ERR_DB_COMPONENT_NOT_FOUND",
            context = json_value!({ "id": id })
        )
    }

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
            description: "Synchronise le composant Arcadia avec le code source physique.".into(),
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
            None => return McpToolResult::error(call.id, "ID manquant"),
        };

        // 1. Fetch & Lang
        let doc = match self.fetch_component(component_id).await {
            Ok(d) => d,
            Err(e) => return McpToolResult::error(call.id, &e.to_string()),
        };

        let lang = match self.determine_language(&doc) {
            Ok(l) => l,
            Err(e) => return McpToolResult::error(call.id, &e.to_string()),
        };

        // 2. Construction du Module
        let name = doc["name"].as_str().unwrap_or("component");
        let mut module = match Module::new(name, PathBuf::from(format!("{}.rs", name))) {
            Ok(m) => m,
            Err(e) => return McpToolResult::error(call.id, &e.to_string()),
        };

        // 3. Analyse & Tissage
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
            body: Some(" { println!(\"RAISE Exec\"); } ".to_string()),
            dependencies: analysis.dependencies,
            metadata: analysis.metadata,
        });

        // 4. Sync disque
        match self.service.sync_module(module).await {
            Ok(path) => {
                if lang == TargetLanguage::Rust {
                    let _ = self.service.format_module(&path);
                }
                McpToolResult::success(call.id, json_value!({ "path": path.to_string_lossy() }))
            }
            Err(e) => McpToolResult::error(call.id, &e.to_string()),
        }
    }
}

// =========================================================================
// 🧪 TESTS UNITAIRES ET D'INTÉGRATION
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::mock::AgentDbSandbox;

    #[async_test]
    async fn test_codegen_tool_execution_flow() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let comp_id = "test-comp-01";

        // 🎯 FIX : Utilisation de l'URI de schéma système injectée par le mock
        let generic_schema_uri = "db://_system/_system/schemas/v1/db/generic.schema.json";

        manager
            .create_collection("pa_components", generic_schema_uri)
            .await
            .expect("Échec création collection de test"); //

        manager
            .upsert_document(
                "pa_components",
                json_value!({
                    "_id": comp_id,
                    "name": "EngineController",
                    "implementation": { "technology": "rust" }
                }),
            )
            .await
            .unwrap();

        // 🎯 FIX : Garantir l'existence physique du dossier pour le service
        let gen_path = sandbox.domain_root.join("src-gen");
        fs::create_dir_all_sync(&gen_path).unwrap(); //

        let tool = CodeGenTool::new(
            gen_path,
            sandbox.db.clone(),
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let call = McpToolCall::new(
            "generate_component_code",
            json_value!({ "component_id": comp_id }),
        );
        let result = tool.execute(call).await;

        // Si erreur, on l'affiche pour faciliter le debug
        if result.is_error {
            panic!("Tool execution failed: {}", result.content["error"]);
        }

        assert!(!result.is_error);
        assert!(result.content["path"]
            .as_str()
            .unwrap()
            .contains("EngineController.rs"));
    }

    #[async_test]
    async fn test_determine_language_logic() {
        let sandbox = AgentDbSandbox::new().await;
        let tool = CodeGenTool::new(PathBuf::from("/tmp"), sandbox.db.clone(), "test", "test");

        let doc_rust = json_value!({ "implementation": { "technology": "rust" } });
        let doc_cpp = json_value!({ "implementation": { "technology": "cpp" } });

        assert_eq!(
            tool.determine_language(&doc_rust).unwrap(),
            TargetLanguage::Rust
        );
        assert_eq!(
            tool.determine_language(&doc_cpp).unwrap(),
            TargetLanguage::Cpp
        );
    }

    #[async_test]
    async fn test_fetch_component_error_handling() {
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
            Err(AppError::Structured(data)) => assert_eq!(data.code, "ERR_DB_COMPONENT_NOT_FOUND"), //
            _ => panic!("Type d'erreur incorrect"),
        }
    }
}
