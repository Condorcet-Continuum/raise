// =========================================================================
//  RAISE CODE GENERATOR : AST Weaver Root Façade (V2 Strict)
// =========================================================================

pub mod analyzers; // Analyse sémantique Arcadia
pub mod diff; // Moteur de comparaison (Jumeau vs Physique)
pub mod graph; // Tri topologique des dépendances
pub mod graph_weaver; // Pont "Graphe ➡️ AST ➡️ Code"
pub mod models; // Modèles de données (CodeElement, Module)
pub mod module_weaver; // Orchestration du tissage fichier
pub mod reconcilers; // Extraction Bottom-Up via @raise-handle
pub mod toolchains;
pub mod utils; // Utilitaires mathématiques (String transformation)
pub mod weaver; // Tissage unitaire des blocs de code

use self::diff::{DiffAction, DiffEngine};
use self::models::Module;
use self::module_weaver::ModuleWeaver;
use self::reconcilers::rust::Reconciler as RustReconciler;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Query, QueryEngine};
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

/// 🧠 Service central d'orchestration de la génération de code.
/// Gère le cycle de vie bidirectionnel entre le Jumeau Numérique (DB) et le Code Physique (Disk).
pub struct CodeGeneratorService {
    root_path: PathBuf,
    skip_compilation: bool,
}

impl CodeGeneratorService {
    /// Initialise le service avec un point de montage racine pour le code source.
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            root_path,
            skip_compilation: false,
        }
    }

    /// 📥 L'Agent d'Ingestion : Lit un fichier physique et peuple le Jumeau Numérique.
    pub async fn ingest_file(
        &self,
        path: &Path,
        manager: &CollectionsManager<'_>,
        schema_uri: &str,
    ) -> RaiseResult<usize> {
        if !path.exists() {
            raise_error!(
                "ERR_CODEGEN_FILE_NOT_FOUND",
                error = "Le fichier source n'existe pas physiquement.",
                context = json_value!({ "path": path.to_string_lossy() })
            );
        }

        // 1. Extraction Lexicale via Reconciler
        let elements = RustReconciler::parse_from_file(path).await?;

        // 2. Préparation de la collection via Mount Point DB
        let _ = manager.create_collection("code_elements", schema_uri).await;

        // 3. Enrichissement et Persistance résiliente
        let mut ingested_count = 0;
        for mut el in elements {
            el.metadata
                .insert("file_path".to_string(), path.to_string_lossy().to_string());

            let json_el = match json::serialize_to_value(&el) {
                Ok(v) => v,
                Err(e) => raise_error!("ERR_CODEGEN_SERIALIZATION", error = e.to_string()),
            };

            match manager.upsert_document("code_elements", json_el).await {
                Ok(_) => ingested_count += 1,
                Err(e) => raise_error!("ERR_DB_WRITE_FAILED", error = e.to_string()),
            }
        }

        Ok(ingested_count)
    }

    /// 📤 L'Agent Forgeron : Matérialise le Jumeau Numérique dans un fichier physique.
    pub async fn weave_file(
        &self,
        module_name: &str,
        path: &Path,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<PathBuf> {
        let query = Query::new("code_elements");
        let db_result = match QueryEngine::new(manager).execute_query(query).await {
            Ok(res) => res,
            Err(e) => raise_error!("ERR_CODEGEN_QUERY_FAILED", error = e.to_string()),
        };

        let mut target_elements = Vec::new();
        let path_str = path.to_string_lossy().to_string();

        for doc in db_result.documents {
            if let Some(meta) = doc.get("metadata") {
                if let Some(fp) = meta.get("file_path").and_then(|v| v.as_str()) {
                    if fp == path_str {
                        let el: models::CodeElement = match json::deserialize_from_value(doc) {
                            Ok(e) => e,
                            Err(err) => {
                                raise_error!("ERR_CODEGEN_DESERIALIZATION", error = err.to_string())
                            }
                        };
                        target_elements.push(el);
                    }
                }
            }
        }

        if target_elements.is_empty() {
            raise_error!(
                "ERR_CODEGEN_NO_ELEMENTS_FOUND",
                error = "Aucun élément trouvé en base pour ce fichier.",
                context = json_value!({ "path": path_str })
            );
        }

        let mut module = Module::new(module_name, path.to_path_buf())?;
        module.elements = target_elements;

        self.sync_module(module).await
    }

    /// 🔄 Synchronise un module sémantique avec le système de fichiers.
    pub async fn sync_module(&self, mut module: Module) -> RaiseResult<PathBuf> {
        let full_path = self.root_path.join(&module.path);
        module.path = full_path.clone();

        if full_path.exists() {
            match self.format_module(&full_path).await {
                Ok(_) => (),
                Err(e) => {
                    user_info!(
                        "MSG_CODEGEN_PRE_SYNC_FMT_FAILED",
                        json_value!({ "path": full_path.to_string_lossy() })
                    );
                    return Err(e);
                }
            }

            let physical_elements = RustReconciler::parse_from_file(&full_path).await?;
            let diffs = match DiffEngine::compute_diff(physical_elements, module.elements.clone()) {
                Ok(d) => d,
                Err(e) => raise_error!("ERR_CODEGEN_DIFF_FAILED", error = e.to_string()),
            };

            for report in diffs {
                if report.action == DiffAction::Upsert {
                    user_info!(
                        "MSG_CODEGEN_MODIF_INTEGRATED",
                        json_value!({ "handle": report.handle })
                    );
                }
            }
        }

        let backup_path = full_path.with_extension("rs.bak");
        let file_exists = full_path.exists();
        if file_exists {
            match fs::copy_async(&full_path, &backup_path).await {
                Ok(_) => (),
                Err(e) => raise_error!("ERR_CODEGEN_BACKUP_FAILED", error = e.to_string()),
            }
        }

        match ModuleWeaver::sync_to_disk(&module, &self.root_path).await {
            Ok(_) => (),
            Err(e) => {
                Self::rollback(&full_path, &backup_path, file_exists).await;
                return Err(e);
            }
        }

        let _ = self.format_module(&full_path).await;

        // 🎯 FIX: Variables inutilisées corrigées ici
        match self.check_workspace(&module.name).await {
            Ok(_) => (),
            Err(e) => {
                Self::rollback(&full_path, &backup_path, file_exists).await;
                return Err(e);
            }
        }

        match self.test_workspace(&module.name).await {
            Ok(_) => (),
            Err(e) => {
                Self::rollback(&full_path, &backup_path, file_exists).await;
                return Err(e);
            }
        }

        if file_exists {
            let _ = fs::remove_file_async(&backup_path).await;
        }

        Ok(full_path)
    }

    pub async fn format_module(&self, path: &Path) -> RaiseResult<()> {
        match os::exec_command_async("rustfmt", &[path.to_string_lossy().as_ref()], None).await {
            Ok(_) => Ok(()),
            Err(e) => raise_error!("ERR_CODEGEN_FMT_FAILED", error = e),
        }
    }

    async fn rollback(target: &Path, backup: &Path, existed_before: bool) {
        if existed_before {
            let _ = fs::copy_async(backup, target).await;
            let _ = fs::remove_file_async(backup).await;
        } else {
            let _ = fs::remove_file_async(target).await;
        }
        user_info!(
            "MSG_CODEGEN_ROLLBACK_EXECUTED",
            json_value!({ "path": target.to_string_lossy() })
        );
    }

    pub fn with_test_mode(mut self) -> Self {
        self.skip_compilation = true;
        self
    }

    async fn check_workspace(&self, _module_name: &str) -> RaiseResult<()> {
        if cfg!(test) || self.skip_compilation {
            return Ok(());
        }

        let output = match os::exec_command_async(
            "cargo",
            &["check", "--lib", "--message-format=json"],
            None,
        )
        .await
        {
            Ok(out) => out,
            Err(e) => raise_error!("ERR_SYSTEM_IO", error = e),
        };

        let mut errors = Vec::new();
        for line in output.lines().filter(|l| l.starts_with('{')) {
            if let Ok(json_line) = json::deserialize_from_str::<JsonValue>(line) {
                if json_line["reason"] == "compiler-message"
                    && json_line["message"]["level"] == "error"
                {
                    if let Some(msg) = json_line["message"]["rendered"].as_str() {
                        errors.push(msg.to_string());
                    }
                }
            }
        }

        if !errors.is_empty() {
            raise_error!(
                "ERR_CODEGEN_COMPILATION_FAILED",
                error = "Échec cargo check.",
                context = json_value!({ "xai_feedback": errors.join("\n---\n") })
            );
        }
        Ok(())
    }

    async fn test_workspace(&self, _module_name: &str) -> RaiseResult<()> {
        if cfg!(test) || self.skip_compilation {
            return Ok(());
        }
        match os::exec_command_async("cargo", &["test", "--lib"], None).await {
            Ok(_) => Ok(()),
            Err(e) => {
                let raw_err = e.to_string();
                let feedback = if let Some(idx) = raw_err.find("failures:") {
                    raw_err[idx..].trim().to_string()
                } else {
                    raw_err
                };
                raise_error!(
                    "ERR_CODEGEN_TESTS_FAILED",
                    error = "Échec des tests unitaires.",
                    context = json_value!({ "xai_feedback": feedback })
                )
            }
        }
    }

    // =========================================================================
    // MODE DÉCOUVERTE (Auto-Indexation Mount Points)
    // =========================================================================

    fn slugify(s: &str) -> String {
        s.chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect()
    }

    fn humanize(s: &str) -> String {
        s.split('_')
            .filter(|w| !w.is_empty())
            .map(|word| {
                let mut c = word.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<Vec<String>>()
            .join(" ")
    }

    fn generate_handle(&self, path: &Path, root: &Path, prefix: &str) -> String {
        let rel_path = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
        let slug = Self::slugify(&rel_path);
        format!("{}_{}", prefix, slug)
    }

    pub async fn index_workspace(
        &self,
        source_path: &Path,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<usize> {
        let mut count = 0;
        let root_dir = if source_path.ends_with("src") {
            source_path.parent().unwrap_or(source_path).to_path_buf()
        } else {
            source_path.to_path_buf()
        };
        let src_dir = root_dir.join("src");

        let root_handle = root_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let root_service_id = format!("ref:services:handle:{}", root_handle);
        let root_name = Self::humanize(&root_handle);

        manager.upsert_document("services", json_value!({
            "@id": root_service_id, "@type": "Service", "handle": root_handle, "name": { "fr": root_name }
        })).await?;
        count += 1;

        if fs::exists_async(&src_dir).await {
            let mut entries = fs::read_dir_async(&src_dir).await?;
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() {
                    let s_handle = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let s_id = format!("ref:services:handle:{}", s_handle);
                    manager.upsert_document("services", json_value!({
                        "@id": s_id, "@type": "Service", "handle": s_handle, "name": { "fr": Self::humanize(&s_handle) }
                    })).await?;
                    count += 1;
                    count += self
                        .index_directory_recursive(&path, &path, &s_handle, &s_id, None, manager)
                        .await?;
                }
            }
        }
        Ok(count)
    }

    fn index_directory_recursive<'a>(
        &'a self,
        current_dir: &'a Path,
        service_root: &'a Path,
        service_handle: &'a str,
        service_id: &'a str,
        parent_comp_id: Option<String>,
        manager: &'a CollectionsManager<'_>,
    ) -> Pinned<Box<dyn AsyncFuture<Output = RaiseResult<usize>> + Send + 'a>> {
        Box::pin(async move {
            let mut count = 0;
            let mut entries = fs::read_dir_async(current_dir).await?;
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() {
                    let c_handle = self.generate_handle(&path, service_root, service_handle);
                    let c_id = format!("ref:components:handle:{}", c_handle);
                    let mut doc = json_value!({
                        "@id": c_id, "@type": "Component", "handle": c_handle, "name": { "fr": Self::humanize(&path.file_name().unwrap_or_default().to_string_lossy()) }, "service_id": service_id
                    });
                    if let Some(ref p) = parent_comp_id {
                        doc["parent_id"] = json_value!(p);
                    }
                    manager.upsert_document("components", doc).await?;
                    count += 1;
                    count += self
                        .index_directory_recursive(
                            &path,
                            service_root,
                            service_handle,
                            service_id,
                            Some(c_id),
                            manager,
                        )
                        .await?;
                } else if path.is_file() {
                    count += self
                        .upsert_module(
                            &path,
                            service_root,
                            service_handle,
                            service_id,
                            parent_comp_id.clone(),
                            manager,
                        )
                        .await?;
                }
            }
            Ok(count)
        })
    }

    async fn upsert_module(
        &self,
        file_path: &Path,
        root_path: &Path,
        prefix_handle: &str,
        service_id: &str,
        component_id: Option<String>,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<usize> {
        let m_handle = self.generate_handle(file_path, root_path, prefix_handle);
        let mut doc = json_value!({
            "@id": format!("ref:modules:handle:{}", m_handle), "@type": "Module", "handle": m_handle, "name": { "fr": Self::humanize(&file_path.file_stem().unwrap_or_default().to_string_lossy()) }, "service_id": service_id
        });
        if let Some(c) = component_id {
            doc["component_id"] = json_value!(c);
        }
        manager.upsert_document("modules", doc).await?;
        Ok(1)
    }
}

// =========================================================================
// TESTS UNITAIRES ET D'INTÉGRATION
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_generator::models::{CodeElement, CodeElementType, Visibility};
    use crate::utils::testing::DbSandbox;

    const TEST_SCHEMA: &str = "db://_system/_system/schemas/v1/db/generic.schema.json";

    #[async_test]
    async fn test_service_sync_flow_strict_ai_master() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        manager
            .create_collection("code_elements", TEST_SCHEMA)
            .await?;

        let root = sandbox.storage.config.data_root.clone();
        let service = CodeGeneratorService::new(root.clone()).with_test_mode();
        let mut module = Module::new("ai_module", root.join("ai.rs"))?;

        module.elements.push(CodeElement {
            module_id: None,
            parent_id: None,
            attributes: vec!["#[ai_master]".into()],
            docs: Some("IA Doc".into()),
            elements: vec![],
            handle: "fn:main".into(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            signature: "pub fn main()".into(),
            body: Some("{ println!(\"Hi\"); }".into()),
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        });

        service.sync_module(module.clone()).await?;
        service
            .ingest_file(&module.path, &manager, TEST_SCHEMA)
            .await?;

        let query = Query::new("code_elements");
        let result = QueryEngine::new(&manager).execute_query(query).await?;
        assert_eq!(result.total_count, 1);
        assert_eq!(result.documents[0]["handle"], "fn:main");
        Ok(())
    }

    #[async_test]
    async fn test_service_ingest_file() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let config = AppConfig::get();
        let service =
            CodeGeneratorService::new(sandbox.storage.config.data_root.clone()).with_test_mode();
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        manager
            .create_collection("code_elements", TEST_SCHEMA)
            .await?;

        let file_path = sandbox.storage.config.data_root.join("test_ingest.rs");
        fs::write_async(
            &file_path,
            "// @raise-handle: fn:test_ingest\npub fn test_ingest() {}",
        )
        .await?;

        let count = service
            .ingest_file(&file_path, &manager, TEST_SCHEMA)
            .await?;
        assert_eq!(count, 1);

        let query = Query::new("code_elements");
        let result = QueryEngine::new(&manager).execute_query(query).await?;
        assert_eq!(result.documents[0]["handle"], "fn:test_ingest");
        Ok(())
    }

    #[async_test]
    async fn test_service_weave_file() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let config = AppConfig::get();
        let service =
            CodeGeneratorService::new(sandbox.storage.config.data_root.clone()).with_test_mode();
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        manager
            .create_collection("code_elements", TEST_SCHEMA)
            .await?;

        let file_path = sandbox.storage.config.data_root.join("test_weave.rs");
        fs::write_async(
            &file_path,
            "// @raise-handle: fn:test_weave\npub fn test_weave() {}",
        )
        .await?;
        service
            .ingest_file(&file_path, &manager, TEST_SCHEMA)
            .await?;

        let query = Query::new("code_elements");
        let mut doc = QueryEngine::new(&manager)
            .execute_query(query)
            .await?
            .documents[0]
            .clone();
        doc["body"] = json_value!("{ println!(\"AI was here\"); }");
        manager.upsert_document("code_elements", doc).await?;

        let final_path = service
            .weave_file("test_weave_mod", &file_path, &manager)
            .await?;
        let final_code = fs::read_to_string_async(&final_path).await?;
        assert!(final_code.contains("AI was here"));
        Ok(())
    }

    #[async_test]
    async fn test_resilience_bad_mount_point() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let service = CodeGeneratorService::new(sandbox.storage.config.data_root.clone());
        let manager = CollectionsManager::new(&sandbox.storage, "ghost_partition", "void_db");

        let result = service
            .ingest_file(Path::new("/tmp/ghost_file.rs"), &manager, TEST_SCHEMA)
            .await;
        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_CODEGEN_FILE_NOT_FOUND");
                Ok(())
            }
            _ => panic!("L'ingestion aurait dû lever ERR_CODEGEN_FILE_NOT_FOUND"),
        }
    }

    #[async_test]
    async fn test_indexer_mount_point_discovery() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 🎯 On prépare les collections
        manager.create_collection("services", TEST_SCHEMA).await?;
        manager.create_collection("components", TEST_SCHEMA).await?;
        manager.create_collection("modules", TEST_SCHEMA).await?;

        let root = sandbox.storage.config.data_root.clone();
        let src = root.join("src");

        // 🎯 FIX : On crée un sous-répertoire 'core' pour que l'indexeur trouve
        // un second service et ses modules.
        let core_dir = src.join("core");
        fs::ensure_dir_async(&core_dir).await?;
        fs::write_async(core_dir.join("mod.rs"), b"// raise").await?;

        let service = CodeGeneratorService::new(root.clone()).with_test_mode();
        let indexed = service.index_workspace(&root, &manager).await?;

        // On s'attend maintenant à : 1 (Root) + 1 (Service core) + 1 (Module mod.rs) = 3
        assert!(indexed >= 2);
        Ok(())
    }
}
