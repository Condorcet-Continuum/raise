use crate::utils::prelude::*;

/// 🚦 États possibles pour le cycle de vie d'un contrat de génération.
/// Alignement strict avec le schéma `staged-contract.schema.json`.
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContractStatus {
    #[default]
    Pending,
    Committed,
    Rejected,
    Expired,
}

/// 📦 Contrat de transition (Staged Contract) représentant une intention de modification.
/// Projection mémoire du nœud `raise:StagedContract` du Jumeau Numérique.
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct StagedModule {
    /// Identifiant public unique du contrat dans le graphe (ex: stage_auth_service)
    pub handle: String,

    /// Identifiant du Mandataire (Agent IA) ou du Mandant (Humain) à l'origine de l'intention
    pub agent_handle: String,

    /// L'état actuel du contrat dans jsondb
    #[serde(default)]
    pub contract_status: ContractStatus,

    /// Le chemin temporaire où le code a été généré pour la validation du compilateur
    pub temp_path: PathBuf,

    /// Le chemin de destination final dans le workspace physique
    pub final_path: PathBuf,

    /// Le nom sémantique du module ciblé (lien vers le module_id)
    pub module_name: String,

    /// L'AST cible proposé par le Mandataire pour calculer le Diff
    pub target_elements: Vec<CodeElement>,
}

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
    use crate::code_generator::module_weaver::ModuleWeaver;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::DbSandbox;

    #[async_test]
    async fn test_staging_persistence_cycle() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let config = AppConfig::get(); // 🎯 Récupération dynamique de la config

        let manager = CollectionsManager::new(
            &sandbox.storage,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 🎯 FIX : Utilisation du schéma générique réservé aux tests
        let generic_schema = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );

        let _ = manager
            .create_collection("staged_contracts", &generic_schema)
            .await;

        // 1. Préparation d'un contrat factice
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

        // 2. Persistance (Écriture)
        ModuleWeaver::persist_stage(&manager, &staged, "agent_smith")
            .await
            .expect("La persistance du contrat a échoué");

        // 3. Chargement (Lecture)
        let loaded = ModuleWeaver::load_stage(&manager, "test_weaver")
            .await
            .expect("Le chargement du contrat a échoué");

        // 4. Assertions strictes
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

        // Tentative de chargement d'un module qui n'existe pas en passant le manager
        let result = ModuleWeaver::load_stage(&manager, "ghost_module").await;

        // Validation que notre façade renvoie bien l'erreur structurée
        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_STAGE_NOT_FOUND");
            }
            _ => panic!("Le chargement aurait dû échouer avec ERR_STAGE_NOT_FOUND"),
        }
        Ok(())
    }

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
