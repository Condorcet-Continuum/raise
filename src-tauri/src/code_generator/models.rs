use crate::utils::prelude::*;

/// 🌐 Langages cibles supportés par l'AST Weaver.
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

/// 📂 Types d'éléments reconnus par l'AST Weaver.
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

/// 🔒 Gestion sémantique de la visibilité.
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Public,
    Private,
    Crate,
    Protected,
}

/// 🧩 L'unité atomique du Jumeau Numérique (Code).
/// 🎯 FIX : Ajout de PartialEq et Eq pour satisfaire la comparaison du Module
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
pub struct CodeElement {
    // 🔗 Liens topologiques
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,

    // 🏷️ Identité
    pub element_type: CodeElementType,
    pub handle: String,
    pub visibility: Visibility,

    // 🧠 Contexte IA
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<String>, // Ex: ["#[cfg(test)]", "#[derive(Debug)]"]

    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>, // Les docstrings (///)

    // ⚙️ Code physique
    pub signature: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    // 🌳 Graphe
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
    pub language: String, // "markdown", "mermaid", "rust", etc.

    #[serde(default)]
    pub elements: Vec<String>,

    #[serde(default, skip_serializing_if = "UnorderedMap::is_empty")]
    pub metadata: UnorderedMap<String, String>,
}

/// 📄 Représentation d'un fichier source physique.
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
pub struct Module {
    pub name: String,
    pub path: PathBuf,
    pub elements: Vec<CodeElement>,
}

impl Module {
    /// Constructeur sécurisé utilisant la gestion d'erreur RAISE.
    pub fn new(name: &str, path: PathBuf) -> RaiseResult<Self> {
        if name.is_empty() {
            // Utilisation de la macro de divergence RAISE
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

    #[test]
    fn test_module_creation_and_serialization() {
        let path = PathBuf::from("src/test.rs");
        let module = Module::new("test_module", path.clone())
            .expect("La création du module ne devrait pas échouer");

        assert_eq!(module.name, "test_module");
        assert_eq!(module.path, path);

        // Test de sérialisation via la façade json
        let json = json::serialize_to_string(&module).unwrap();
        assert!(json.contains("test_module"));
    }

    #[test]
    fn test_module_error_handling() {
        // Test du flux de contrôle de raise_error!
        let result = Module::new("", PathBuf::from("/tmp"));

        assert!(result.is_err());
        if let Err(AppError::Structured(data)) = result {
            assert_eq!(data.code, "ERR_CODEGEN_INVALID_NAME");
            assert_eq!(data.service, "code_generator"); // Auto-détecté par la macro
        }
    }

    #[test]
    fn test_code_element_dependencies() {
        let mut metadata = UnorderedMap::new();
        metadata.insert("layer".to_string(), "Physical".to_string());

        let element = CodeElement {
            // 🎯 FIX : Ajout des nouveaux champs manquants pour que le test compile !
            module_id: None,
            parent_id: None,
            attributes: vec![],
            docs: None,
            elements: vec![],

            // Anciens champs
            handle: "fn_core".to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            signature: "pub fn core()".to_string(),
            body: Some("{}".to_string()),
            dependencies: vec!["struct_config".to_string()],
            metadata,
        };

        assert_eq!(element.dependencies.len(), 1);
        assert_eq!(element.metadata.get("layer").unwrap(), "Physical");
    }
}
