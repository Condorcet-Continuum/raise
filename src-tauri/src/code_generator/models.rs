use crate::utils::prelude::*;

/// 🌐 Langages cibles supportés par l'AST Weaver.
/// Déplacé ici pour être accessible globalement dans le module.
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
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    ModuleDeclaration,
    TypeAlias,
    Constant,
    ModuleHeader, // Pour les imports (use) et macros globales
    TestModule,   // Pour le bloc #[cfg(test)]
}

/// 🔒 Gestion sémantique de la visibilité.
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Public,
    Private,
    Crate,
    Restricted(String),
}

/// 🧩 L'unité atomique du Jumeau Numérique (Code).
/// Utilise les alias RAISE pour la cohérence IA.
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct CodeElement {
    /// Identifiant sémantique unique (ex: "ref:functions:engine_start").
    pub handle: String,
    pub element_type: CodeElementType,
    pub visibility: Visibility,
    /// Signature brute pour le moteur 'syn'.
    pub signature: String,
    /// Corps optionnel (None pour les interfaces/traits).
    pub body: Option<String>,
    /// Liste des handles requis pour le tri topologique.
    pub dependencies: Vec<String>,
    /// Métadonnées Arcadia (ex: "arcadia_layer": "LA").
    pub metadata: UnorderedMap<String, String>,
}

/// 📄 Représentation d'un fichier source physique.
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct Module {
    /// Utilisation de UniqueId (Alias Uuid).
    pub id: UniqueId,
    pub name: String,
    /// Éléments contenus dans le module.
    pub elements: Vec<CodeElement>,
    pub path: PathBuf, // Alias sémantique
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
            id: UniqueId::new_v4(),
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
