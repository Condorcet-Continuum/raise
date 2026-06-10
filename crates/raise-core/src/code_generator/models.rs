use crate::utils::prelude::*;

/// 🚦 États possibles pour le cycle de vie d'un contrat de génération.
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContractStatus {
    #[default]
    Pending,
    Committed,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct StagedModule {
    pub handle: String,
    pub agent_handle: String,
    #[serde(default)]
    pub contract_status: ContractStatus,
    pub temp_path: PathBuf,
    pub final_path: PathBuf,
    pub module_name: String,
    pub target_elements: Vec<CodeElement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serializable, Deserializable)]
#[serde(rename_all = "snake_case")]
pub enum TargetLanguage {
    Rust,
    TypeScript,
    Python,
    Cpp,
    Verilog,
    Vhdl,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodeElementType {
    ImportBlock,
    Function,
    TestModule,
    TestFunction,
    ImplBlock,
    Trait,
    Struct,
    Enum,
    Constant,
    Macro,
    TypeAlias,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Public,
    Private,
    Crate,
    Protected,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
pub struct CodeElement {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub element_type: CodeElementType,
    pub handle: String,
    pub visibility: Visibility,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
    pub signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    #[serde(default, skip_serializing_if = "UnorderedMap::is_empty")]
    pub metadata: UnorderedMap<String, String>,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocElementType {
    MarkdownSection,
    Frontmatter,
    MermaidDiagram,
    CodeBlock,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
pub struct DocElement {
    pub module_id: Option<String>,
    pub parent_id: Option<String>,
    pub element_type: DocElementType,
    pub handle: String,
    pub title: String,
    pub heading_level: Option<u32>,
    pub content: String,
    pub language: String,
    #[serde(default)]
    pub elements: Vec<String>,
    #[serde(default, skip_serializing_if = "UnorderedMap::is_empty")]
    pub metadata: UnorderedMap<String, String>,
}

// =========================================================================
// 🎯 INJECTION ZÉRO DETTE : Modélisation des JSON Schema Elements
// =========================================================================

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SchemaType {
    Object,
    Array,
    String,
    Number,
    Boolean,
    Null,
    Multi,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompositionStrategy {
    #[default]
    None,
    AllOf,
    AnyOf,
    OneOf,
    Not,
}

/// Modélisation stricte de l'atome d'un contrat JSON Schema.
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
pub struct JsonSchemaElement {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub handle: String,
    #[serde(default = "default_draft")]
    pub draft: String,
    pub schema_type: SchemaType,
    #[serde(default)]
    pub composition_strategy: CompositionStrategy,
    pub content: JsonValue,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_dependencies: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_binding: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_config: Option<JsonValue>,
    #[serde(default, skip_serializing_if = "UnorderedMap::is_empty")]
    pub metadata: UnorderedMap<String, String>,
}

fn default_draft() -> String {
    "2020-12".to_string()
}

// =========================================================================

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
pub struct Module {
    pub name: String,
    pub path: PathBuf,
    pub elements: Vec<CodeElement>,
}

impl Module {
    pub fn new(name: &str, path: PathBuf) -> RaiseResult<Self> {
        if name.is_empty() {
            raise_error!(
                "ERR_CODEGEN_INVALID_NAME",
                error = "Le nom du module ne peut pas être vide",
                context = json_value!({ "path": path.to_string_lossy() })
            );
        }
        Ok(Self {
            name: name.to_string(),
            elements: Vec::new(),
            path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_generator::module_weaver::ModuleWeaver;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::DbSandbox;

    #[async_test]
    async fn test_staging_persistence_cycle() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        let generic_schema = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );
        let _ = manager
            .create_collection("staged_contracts", &generic_schema)
            .await;

        let staged = StagedModule {
            handle: "stage_test_weaver".to_string(),
            agent_handle: "agent_smith".to_string(),
            contract_status: ContractStatus::Pending,
            temp_path: sandbox.storage.config.data_root.join("temp.rs"),
            final_path: sandbox.storage.config.data_root.join("final.rs"),
            module_name: "test_weaver".to_string(),
            target_elements: vec![CodeElement {
                module_id: None,
                parent_id: None,
                attributes: vec![],
                docs: None,
                elements: vec![],
                handle: "fn:test".to_string(),
                element_type: CodeElementType::Function,
                visibility: Visibility::Public,
                signature: "fn test()".to_string(),
                body: Some("{}".to_string()),
                dependencies: vec![],
                metadata: UnorderedMap::new(),
            }],
        };

        ModuleWeaver::persist_stage(&manager, &staged, "agent_smith").await?;
        let loaded = ModuleWeaver::load_stage(&manager, "test_weaver").await?;

        assert_eq!(loaded.module_name, staged.module_name);
        assert_eq!(loaded.target_elements.len(), 1);
        assert_eq!(loaded.target_elements[0].handle, "fn:test");
        Ok(())
    }

    #[async_test]
    async fn test_load_non_existent_stage_fails() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        let generic_schema = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );
        let _ = manager
            .create_collection("staged_contracts", &generic_schema)
            .await;

        let result = ModuleWeaver::load_stage(&manager, "ghost_module").await;
        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_STAGE_NOT_FOUND");
            }
            _ => panic!("Le chargement aurait dû échouer avec ERR_STAGE_NOT_FOUND"),
        }
        Ok(())
    }

    #[test]
    fn test_module_creation_and_serialization() -> RaiseResult<()> {
        let path = PathBuf::from("src/test.rs");
        let module = Module::new("test_module", path.clone())?;

        assert_eq!(module.name, "test_module");
        assert_eq!(module.path, path);

        let json = match json::serialize_to_string(&module) {
            Ok(j) => j,
            Err(_) => raise_error!("ERR_TEST_FAIL", error = "Sérialisation impossible"),
        };
        assert!(json.contains("test_module"));
        Ok(())
    }

    #[test]
    fn test_module_error_handling() {
        let result = Module::new("", PathBuf::from("/tmp"));
        assert!(result.is_err());
        if let Err(AppError::Structured(data)) = result {
            assert_eq!(data.code, "ERR_CODEGEN_INVALID_NAME");
            assert_eq!(data.service, "code_generator");
        }
    }

    #[test]
    fn test_code_element_dependencies() {
        let mut metadata = UnorderedMap::new();
        metadata.insert("layer".to_string(), "Physical".to_string());
        let element = CodeElement {
            module_id: None,
            parent_id: None,
            attributes: vec![],
            docs: None,
            elements: vec![],
            handle: "fn_core".to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            signature: "pub fn core()".to_string(),
            body: Some("{}".to_string()),
            dependencies: vec!["struct_config".to_string()],
            metadata,
        };
        assert_eq!(element.dependencies.len(), 1);
        assert_eq!(
            element.metadata.get("layer").map(|s| s.as_str()),
            Some("Physical")
        );
    }
}
