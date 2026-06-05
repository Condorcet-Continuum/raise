// FICHIER : crates/raise-core/src/code_generator/mod.rs

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
use self::models::{Module, TargetLanguage};
use self::module_weaver::ModuleWeaver;
use self::reconcilers::markdown::DocReconciler;
use self::reconcilers::rust::Reconciler as RustReconciler;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Query, QueryEngine};
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

#[derive(Clone, Debug)]
pub struct SemanticRoute {
    pub aliases: Vec<String>,
    pub collection: String,
    pub schema_uri: String,
}

/// 🧠 Service central d'orchestration de la génération de code.
/// Gère le cycle de vie bidirectionnel entre le Jumeau Numérique (DB) et le Code Physique (Disk).
pub struct CodeGeneratorService {
    root_path: PathBuf,
    skip_compilation: bool,
    pub semantic_routing: UnorderedMap<String, SemanticRoute>,
}

impl CodeGeneratorService {
    /// Initialise le service avec un point de montage racine et charge sa configuration (Zéro Dette).
    pub async fn new(root_path: PathBuf, manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        let settings = match AppConfig::get_runtime_settings(
            manager,
            "ref:components:handle:codegen_engine",
        )
        .await
        {
            Ok(s) => s,
            Err(e) => raise_error!(
                "ERR_CODEGEN_INIT_REJECTED",
                error = e.to_string(),
                context = json_value!({"action": "codegen_init", "hint": "Le composant codegen_engine est-il actif et configuré dans le catalogue système ?"})
            ),
        };

        let routing_json = settings
            .get("semantic_routing")
            .and_then(|v| v.as_object())
            .ok_or_else(|| {
                build_error!(
                    "ERR_CODEGEN_CONFIG_INVALID",
                    error = "Le paramètre 'semantic_routing' est strictement requis."
                )
            })?;

        let mut semantic_routing = UnorderedMap::new();
        for (key, route) in routing_json {
            let aliases = route
                .get("aliases")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let collection = route
                .get("collection")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let schema_uri = route
                .get("schema_uri")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            semantic_routing.insert(
                key.clone(),
                SemanticRoute {
                    aliases,
                    collection,
                    schema_uri,
                },
            );
        }

        Ok(Self {
            root_path,
            skip_compilation: false,
            semantic_routing,
        })
    }

    /// Résout dynamiquement la collection et le schéma cible (Fail-Fast)
    pub fn get_route(&self, domain: &str) -> RaiseResult<(&str, &str)> {
        let query = domain.to_lowercase();
        for (key, route) in &self.semantic_routing {
            if key == &query || route.aliases.iter().any(|a| a == &query) {
                return Ok((&route.collection, &route.schema_uri));
            }
        }
        raise_error!(
            "ERR_CODEGEN_UNSUPPORTED_DOMAIN",
            error = format!(
                "Le domaine sémantique '{}' n'est pas déclaré dans le catalogue.",
                domain
            )
        )
    }

    /// 🚀 L'Agent Forgeron (Top-Down) : Génère un fichier physique à partir d'un élément du modèle.
    pub async fn generate(
        &self,
        module_doc: JsonValue, // 🎯 Uniformisation de la signature
        element_id: &str,
        manager: &CollectionsManager<'_>,
        lang: TargetLanguage,
    ) -> RaiseResult<PathBuf> {
        let domain = match lang {
            TargetLanguage::Rust
            | TargetLanguage::TypeScript
            | TargetLanguage::Cpp
            | TargetLanguage::Python => "software",
            TargetLanguage::Verilog | TargetLanguage::Vhdl => "hardware",
        };
        let (collection, _) = self.get_route(domain)?;

        let query = Query::new(collection);
        let db_result = match QueryEngine::new(manager).execute_query(query).await {
            Ok(res) => res,
            Err(e) => raise_error!("ERR_CODEGEN_QUERY_FAILED", error = e.to_string()),
        };

        let mut found_doc = None;
        for doc in db_result.documents {
            if let Some(handle) = doc.get("handle").and_then(|v| v.as_str()) {
                if handle == element_id {
                    found_doc = Some(doc);
                    break;
                }
            }
        }

        let doc = match found_doc {
            Some(d) => d,
            None => raise_error!(
                "ERR_CODEGEN_ELEMENT_NOT_FOUND",
                error = "Élément introuvable dans le graphe sémantique.",
                context = json_value!({ "element_id": element_id, "collection": collection })
            ),
        };

        let element: models::CodeElement = match json::deserialize_from_value(doc) {
            Ok(e) => e,
            Err(err) => raise_error!("ERR_CODEGEN_DESERIALIZATION", error = err.to_string()),
        };

        let ext = match lang {
            TargetLanguage::Rust => "rs",
            TargetLanguage::TypeScript => "ts",
            TargetLanguage::Cpp => "cpp",
            TargetLanguage::Verilog => "v",
            TargetLanguage::Vhdl => "vhd",
            TargetLanguage::Python => "py",
        };

        let path_str = module_doc
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let module_name = module_doc
            .get("handle")
            .and_then(|v| v.as_str())
            .unwrap_or(element_id);

        let target_path = if path_str.is_empty() {
            PathBuf::from(&format!("{}.{}", element_id.replace(':', "_"), ext))
        } else {
            PathBuf::from(path_str)
        };

        let mut module = Module::new(module_name, target_path)?;
        module.elements.push(element);

        self.sync_module(module).await
    }

    /// 📥 L'Agent d'Ingestion : Lit un fichier physique et peuple le Jumeau Numérique à partir du contexte module.
    pub async fn ingest_module(
        &self,
        module_doc: JsonValue,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<usize> {
        // 🎯 IMPORT OBLIGATOIRE POUR L'ARCHITECTURE ZERO DETTE
        use crate::json_db::transactions::{manager::TransactionManager, TransactionRequest};

        let path_str = match module_doc.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => raise_error!(
                "ERR_CODEGEN_MODULE_NO_PATH",
                error = "Le nœud module ne possède pas de chemin physique 'path'."
            ),
        };
        let path = Path::new(path_str);

        let module_id = match module_doc.get("_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => raise_error!(
                "ERR_CODEGEN_INGESTION_REJECTED",
                error = "L'ingestion a été avortée : '_id' invalide."
            ),
        };

        if !path.exists() {
            raise_error!(
                "ERR_CODEGEN_FILE_NOT_FOUND",
                error = "Le fichier source n'existe pas physiquement."
            );
        }

        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // 🎯 1. FACTORISATION : On récupère les éléments de manière agnostique
        let (raw_elements, collection, schema_uri) = match extension {
            "rs" | "cpp" | "ts" => {
                let (col, uri) = self.get_route("software")?;
                let els = RustReconciler::parse_from_file(path, module_id).await?;

                // ⚠️ On map dynamiquement tes éléments métiers vers JsonValue
                let json_els = els
                    .into_iter()
                    .map(|mut el| {
                        el.metadata
                            .insert("file_path".to_string(), path_str.to_string());
                        crate::utils::data::json::serialize_to_value(&el).unwrap()
                    })
                    .collect::<Vec<_>>();

                (json_els, col, uri)
            }
            "md" => {
                let (col, uri) = self.get_route("doc")?;
                let els = DocReconciler::parse_from_file(path, module_id).await?;

                let json_els = els
                    .into_iter()
                    .map(|mut el| {
                        el.metadata
                            .insert("file_path".to_string(), path_str.to_string());
                        crate::utils::data::json::serialize_to_value(&el).unwrap()
                    })
                    .collect::<Vec<_>>();

                (json_els, col, uri)
            }
            _ => {
                crate::user_warn!(
                    "MSG_CODEGEN_UNSUPPORTED_EXT",
                    json_value!({ "path": path_str, "extension": extension })
                );
                return Ok(0);
            }
        };

        let _ = manager.create_collection(collection, schema_uri).await;

        // 🎯  Traitement Transactionnel par Lot
        let tx_mgr = TransactionManager::new(manager.storage, &manager.space, &manager.db);
        let mut ops = Vec::new();

        for json_el in raw_elements {
            let handle = json_el
                .get("handle")
                .and_then(|h| h.as_str())
                .map(|s| s.to_string());

            // On délègue l'Upsert au moteur intelligent qui va résoudre les ref:... !
            ops.push(TransactionRequest::Upsert {
                collection: collection.to_string(),
                id: None,
                handle,
                document: json_el,
            });
        }

        let ingested_count = ops.len();
        if ingested_count > 0 {
            tx_mgr.execute_smart(ops).await?;
        }

        Ok(ingested_count)
    }

    /// 📤 L'Agent Forgeron (Tissage) : Reconstitue le fichier de code à partir de la DB.
    pub async fn weave_module(
        &self,
        module_doc: JsonValue,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<PathBuf> {
        let path_str = match module_doc.get("path").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => raise_error!(
                "ERR_CODEGEN_MODULE_NO_PATH",
                error = "Le nœud module ne possède pas de chemin physique 'path'."
            ),
        };
        let module_name = match module_doc.get("handle").and_then(|v| v.as_str()) {
            Some(h) => h.to_string(),
            None => raise_error!(
                "ERR_CODEGEN_MODULE_NO_HANDLE",
                error = "Le nœud module ne possède pas de 'handle' sémantique."
            ),
        };

        let path = Path::new(&path_str);
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let domain = match extension {
            "md" => "doc",
            "json" => "schema",
            _ => "software",
        };
        let (collection, _) = self.get_route(domain)?;

        let query = Query::new(collection);
        let db_result = match QueryEngine::new(manager).execute_query(query).await {
            Ok(res) => res,
            Err(e) => raise_error!("ERR_CODEGEN_QUERY_FAILED", error = e.to_string()),
        };

        let mut target_elements = Vec::new();

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
                context = json_value!({ "path": path_str, "module": module_name })
            );
        }

        let mut module = Module::new(&module_name, path.to_path_buf())?;
        module.elements = target_elements;

        self.sync_module(module).await
    }

    /// 🧬 L'Agent d'Auto-Tagging : Injection et alignement des ancres AST.
    pub async fn auto_tag_module(&self, module_doc: JsonValue) -> RaiseResult<usize> {
        RustReconciler::auto_tag_module(&module_doc).await
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
            let module_id = format!("ref:modules:handle:{}", module.name);
            let physical_elements = RustReconciler::parse_from_file(&full_path, module_id).await?;
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
            "@id": format!("ref:modules:handle:{}", m_handle), "@type": "Module", "handle": m_handle, "name": { "fr": Self::humanize(&file_path.file_stem().unwrap_or_default().to_string_lossy()) }, "service_id": service_id,
            "path": file_path.to_string_lossy().to_string() // 🎯 Essentiel pour le cycle sémantique !
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

    /// 🎯 HELPER ZÉRO DETTE
    async fn inject_mock_codegen_config(manager: &CollectionsManager<'_>) -> RaiseResult<()> {
        let config = AppConfig::get();
        let generic_schema = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );
        let _ = DbSandbox::mock_db(manager).await;

        let _ = manager.create_collection("services", &generic_schema).await;
        let _ = manager
            .create_collection("components", &generic_schema)
            .await;
        let _ = manager.create_collection("modules", &generic_schema).await;
        let _ = manager
            .create_collection("service_configs", &generic_schema)
            .await;

        manager.upsert_document("components", json_value!({ "_id": "ref:components:handle:codegen_engine", "handle": "codegen_engine" })).await?;
        manager.upsert_document("service_configs", json_value!({
            "_id": "mock_codegen",
            "component_id": "ref:components:handle:codegen_engine",
            "service_settings": {
                "format_on_save": true,
                "strict_mode": true,
                "semantic_routing": {
                    "software": { "aliases": ["rust", "cpp", "ts"], "collection": "code_elements", "schema_uri": generic_schema.clone() },
                    "doc": { "aliases": ["md"], "collection": "doc_elements", "schema_uri": generic_schema.clone() }
                }
            }
        })).await?;
        Ok(())
    }

    #[async_test]
    async fn test_service_sync_flow_strict_ai_master() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        inject_mock_codegen_config(&manager).await?;

        manager
            .create_collection("code_elements", TEST_SCHEMA)
            .await?;

        let root = sandbox.storage.config.data_root.clone();
        let service = CodeGeneratorService::new(root.clone(), &manager)
            .await?
            .with_test_mode();
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

        let mock_module_doc = json_value!({
            "_id": "ai_module",
            "handle": "ai_module",
            "path": module.path.to_string_lossy().to_string()
        });

        service.ingest_module(mock_module_doc, &manager).await?;

        let query = Query::new("code_elements");
        let result = QueryEngine::new(&manager).execute_query(query).await?;
        assert_eq!(result.total_count, 1);
        assert_eq!(result.documents[0]["handle"], "fn:main");
        Ok(())
    }

    #[async_test]
    async fn test_service_ingest_module() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        inject_mock_codegen_config(&manager).await?;
        manager
            .create_collection("code_elements", TEST_SCHEMA)
            .await?;

        let service = CodeGeneratorService::new(sandbox.storage.config.data_root.clone(), &manager)
            .await?
            .with_test_mode();

        let file_path = sandbox.storage.config.data_root.join("test_ingest.rs");
        fs::write_async(
            &file_path,
            "// @raise-handle: fn:test_ingest\npub fn test_ingest() {}",
        )
        .await?;

        let mock_module_doc = json_value!({
            "_id": "test_ingest_mod",
            "handle": "test_ingest_mod",
            "path": file_path.to_string_lossy().to_string()
        });

        let count = service.ingest_module(mock_module_doc, &manager).await?;
        assert_eq!(count, 1);

        let query = Query::new("code_elements");
        let result = QueryEngine::new(&manager).execute_query(query).await?;
        assert_eq!(result.documents[0]["handle"], "fn:test_ingest");
        Ok(())
    }

    #[async_test]
    async fn test_service_weave_module() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        inject_mock_codegen_config(&manager).await?;
        manager
            .create_collection("code_elements", TEST_SCHEMA)
            .await?;

        let service = CodeGeneratorService::new(sandbox.storage.config.data_root.clone(), &manager)
            .await?
            .with_test_mode();

        let file_path = sandbox.storage.config.data_root.join("test_weave.rs");
        fs::write_async(
            &file_path,
            "// @raise-handle: fn:test_weave\npub fn test_weave() {}",
        )
        .await?;

        let mock_module_doc = json_value!({
            "_id": "test_weave_mod",
            "handle": "test_weave_mod",
            "path": file_path.to_string_lossy().to_string()
        });

        service
            .ingest_module(mock_module_doc.clone(), &manager)
            .await?;

        let query = Query::new("code_elements");
        let mut doc = QueryEngine::new(&manager)
            .execute_query(query)
            .await?
            .documents[0]
            .clone();
        doc["body"] = json_value!("{ println!(\"AI was here\"); }");
        manager.upsert_document("code_elements", doc).await?;

        let final_path = service.weave_module(mock_module_doc, &manager).await?;
        let final_code = fs::read_to_string_async(&final_path).await?;
        assert!(final_code.contains("AI was here"));
        Ok(())
    }

    #[async_test]
    async fn test_resilience_bad_mount_point() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;

        // Le manager pointe vers une DB qui n'a pas la config de routage
        let manager = CollectionsManager::new(&sandbox.storage, "ghost_partition", "void_db");

        let result =
            CodeGeneratorService::new(sandbox.storage.config.data_root.clone(), &manager).await;
        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_CODEGEN_INIT_REJECTED");
                Ok(())
            }
            _ => panic!("L'initialisation aurait dû échouer faute de configuration système."),
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
        inject_mock_codegen_config(&manager).await?;

        // Les collections sont déjà créées par inject_mock_codegen_config

        let root = sandbox.storage.config.data_root.clone();
        let src = root.join("src");

        let core_dir = src.join("core");
        fs::ensure_dir_async(&core_dir).await?;
        fs::write_async(core_dir.join("mod.rs"), b"// raise").await?;

        let service = CodeGeneratorService::new(root.clone(), &manager)
            .await?
            .with_test_mode();
        let indexed = service.index_workspace(&root, &manager).await?;

        assert!(indexed >= 2);
        Ok(())
    }
}
