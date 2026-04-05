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
use crate::utils::prelude::*;

/// 🧠 Service central d'orchestration de la génération de code.
/// Remplace l'ancien système basé sur Tera et les Injections Points.
pub struct CodeGeneratorService {
    root_path: PathBuf,
    skip_compilation: bool,
}

impl CodeGeneratorService {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            root_path,
            skip_compilation: false,
        }
    }

    /// 📥 L'Agent d'Ingestion : Lit un fichier physique et peuple le Jumeau Numérique
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

        // 1. Extraction Lexicale
        let elements = RustReconciler::parse_from_file(path).await?;

        // 2. Préparation de la collection
        let _ = manager.create_collection("code_elements", schema_uri).await;

        // 3. Enrichissement et Sauvegarde
        let mut ingested_count = 0;
        for mut el in elements {
            el.metadata
                .insert("file_path".to_string(), path.to_string_lossy().to_string());
            let json_el = json::serialize_to_value(&el)?;
            manager.upsert_document("code_elements", json_el).await?;
            ingested_count += 1;
        }

        Ok(ingested_count)
    }

    /// 📤 L'Agent Forgeron : Matérialise le Jumeau Numérique dans un fichier physique
    pub async fn weave_file(
        &self,
        module_name: &str,
        path: &Path,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<PathBuf> {
        let query = Query::new("code_elements");
        let db_result = QueryEngine::new(manager).execute_query(query).await?;

        let mut target_elements = Vec::new();
        let path_str = path.to_string_lossy().to_string();

        for doc in db_result.documents {
            if let Some(meta) = doc.get("metadata") {
                if let Some(fp) = meta.get("file_path").and_then(|v| v.as_str()) {
                    if fp == path_str {
                        let el: models::CodeElement = json::deserialize_from_value(doc)?;
                        target_elements.push(el);
                    }
                }
            }
        }

        if target_elements.is_empty() {
            raise_error!(
                "ERR_CODEGEN_NO_ELEMENTS_FOUND",
                error = "Aucun élément trouvé en base pour ce fichier. Avez-vous ingéré le fichier d'abord ?",
                context = json_value!({ "path": path_str })
            );
        }

        let mut module = Module::new(module_name, path.to_path_buf())?;
        module.elements = target_elements;

        // Appel à notre boucle transactionnelle blindée
        self.sync_module(module).await
    }

    /// 🔄 Synchronise un module sémantique avec le système de fichiers.
    /// Flux : Réconciliation (Bottom-Up) -> Diffing -> Fusion -> Tissage (Top-Down).
    pub async fn sync_module(&self, mut module: Module) -> RaiseResult<PathBuf> {
        let full_path = self.root_path.join(&module.path);
        module.path = full_path.clone();

        // 1. PHASE BOTTOM-UP : Lecture de la réalité physique
        if full_path.exists() {
            // 🆕 PHASE 0 : Normalisation du code existant via rustfmt
            // Cela garantit que le code lu par le Reconciler a une structure standardisée.
            if let Err(e) = self.format_module(&full_path).await {
                user_info!(
                    "MSG_CODEGEN_PRE_SYNC_FMT_FAILED",
                    json_value!({ "path": full_path.to_string_lossy(), "error": e.to_string() })
                );
                // Si rustfmt échoue, c'est que l'Agent a généré du code syntaxiquement invalide.
                // On bloque la synchronisation pour protéger le Jumeau Numérique.
                return Err(e);
            }

            let physical_elements = match RustReconciler::parse_from_file(&full_path).await {
                Ok(elems) => elems,
                Err(e) => return Err(e),
            };

            // 2. PHASE DIFFING : Comparaison avec le Jumeau Numérique (module.elements)
            let diffs = match DiffEngine::compute_diff(
                physical_elements.clone(),
                module.elements.clone(),
            ) {
                Ok(d) => d,
                Err(e) => return Err(e),
            };

            // 3. PHASE FUSION : On intègre les modifications (issues de l'IA ou physiques)
            for report in diffs {
                if report.action == DiffAction::Upsert {
                    for phys_el in &physical_elements {
                        if phys_el.handle == report.handle {
                            // 🎯 FIX : On ne remplace PLUS les éléments du module par ceux du disque.
                            // On se contente de loguer ce que l'Agent s'apprête à écraser.
                            // self.update_element_in_module(&mut module, phys_el.clone());

                            user_info!(
                                "MSG_CODEGEN_MODIF_INTEGRATED",
                                json_value!({ "handle": report.handle, "reason": report.reason })
                            );
                        }
                    }
                }
            }
        }

        // 4. PHASE TOP-DOWN & BOUCLE TRANSACTIONNELLE

        // 4.1 Création du Backup
        let backup_path = full_path.with_extension("rs.bak");
        let file_exists = full_path.exists();
        if file_exists {
            if let Err(e) = fs::copy_async(&full_path, &backup_path).await {
                raise_error!(
                    "ERR_CODEGEN_BACKUP_FAILED",
                    error = e.to_string(),
                    context = json_value!({ "path": full_path.to_string_lossy() })
                );
            }
        }

        // 4.2 Tissage Top-Down
        if let Err(e) = ModuleWeaver::sync_to_disk(&module, &self.root_path).await {
            Self::rollback(&full_path, &backup_path, file_exists).await;
            return Err(e); // L'erreur de tissage remonte
        }

        // 4.3 Formatage de propreté
        let _ = self.format_module(&full_path).await;

        // 4.4 Compilation stricte (cargo check)
        if let Err(e) = self.check_workspace(&module.name).await {
            Self::rollback(&full_path, &backup_path, file_exists).await;
            return Err(e); // Propage l'erreur structurée (avec logs cargo) à l'IA
        }

        // 4.5 Exécution des tests (cargo test)
        if let Err(e) = self.test_workspace(&module.name).await {
            Self::rollback(&full_path, &backup_path, file_exists).await;
            return Err(e); // Propage l'erreur structurée (avec logs de test) à l'IA
        }

        // 4.6 Validation de la transaction (Commit)
        if file_exists {
            let _ = fs::remove_file_async(&backup_path).await;
        }

        Ok(full_path)
    }

    /// 🧹 Post-process : Formatage du code (Anciennement lié à Clippy).
    pub async fn format_module(&self, path: &Path) -> RaiseResult<()> {
        match os::exec_command_async("rustfmt", &[path.to_string_lossy().as_ref()], None).await {
            Ok(_) => Ok(()),
            Err(e) => raise_error!(
                "ERR_CODEGEN_FMT_FAILED",
                error = e,
                context = json_value!({ "path": path.to_string_lossy() })
            ),
        }
    }
    /// ⏪ Restaure le fichier dans son état précédent en cas d'échec de l'IA
    async fn rollback(target: &Path, backup: &Path, existed_before: bool) {
        if existed_before {
            let _ = fs::copy_async(backup, target).await;
            let _ = fs::remove_file_async(backup).await;
        } else {
            // Si le fichier n'existait pas du tout avant, on le supprime simplement
            let _ = fs::remove_file_async(target).await;
        }
        user_info!(
            "MSG_CODEGEN_ROLLBACK_EXECUTED",
            json_value!({ "path": target.to_string_lossy() })
        );
    }

    /// 🛠️ Active le mode test (désactive cargo check/test pour éviter le verrouillage)
    pub fn with_test_mode(mut self) -> Self {
        self.skip_compilation = true;
        self
    }

    /// ⚖️ Le Juge de Paix : Vérifie que le projet compile et structure le feedback pour l'IA
    async fn check_workspace(&self, module_name: &str) -> RaiseResult<()> {
        if cfg!(test) || self.skip_compilation {
            return Ok(());
        }

        // 1. Exécution via la façade OS stricte
        let output = match os::exec_command_async(
            "cargo",
            &["check", "--lib", "--message-format=json"],
            None,
        )
        .await
        {
            Ok(out) => out,
            Err(e) => {
                // 🛑 STRICT : Utilisation de raise_error! pour capturer l'erreur OS dans le "Trou noir" technique
                raise_error!(
                    "ERR_SYSTEM_IO",
                    error = e,
                    context = json_value!({
                        "action": "cargo_check",
                        "module": module_name
                    })
                );
            }
        };

        let mut error_messages = Vec::new();

        // 2. Parsing du retour de Cargo via la façade JSON
        for line in output.lines() {
            if line.starts_with('{') {
                // STRICT : Utilisation de json::deserialize_from_string et JsonValue
                if let Ok(json_line) = json::deserialize_from_str::<JsonValue>(line) {
                    if json_line.get("reason").and_then(|v| v.as_str()) == Some("compiler-message")
                    {
                        if let Some(message_obj) = json_line.get("message") {
                            if message_obj.get("level").and_then(|v| v.as_str()) == Some("error") {
                                if let Some(rendered) =
                                    message_obj.get("rendered").and_then(|v| v.as_str())
                                {
                                    error_messages.push(rendered.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Gestion de l'échec (World Model Feedback)
        if !error_messages.is_empty() {
            let feedback_payload = error_messages.join("\n---\n");

            // STRICT : Notification IHM via la macro
            user_warn!(
                "MSG_CODEGEN_COMPILATION_FAILED",
                json_value!({ "module": module_name, "errors_count": error_messages.len() })
            );

            // STRICT : Levée de l'erreur AI-Ready
            raise_error!(
                "ERR_CODEGEN_COMPILATION_FAILED",
                error = "La compilation Cargo a échoué. Le modèle IA doit corriger le code.",
                context = json_value!({
                    "module": module_name,
                    "xai_feedback": feedback_payload,
                    "status": "requires_ai_correction"
                })
            );
        }

        // 4. Succès
        user_success!(
            "MSG_CODEGEN_COMPILATION_SUCCESS",
            json_value!({ "module": module_name })
        );

        Ok(())
    }

    /// 🧪 L'Épreuve du Feu : Exécute les tests unitaires et structure les échecs pour l'IA
    async fn test_workspace(&self, module_name: &str) -> RaiseResult<()> {
        // On évite les boucles infinies si on est déjà en train de tester le moteur lui-même
        if cfg!(test) || self.skip_compilation {
            return Ok(());
        }

        // 1. Exécution des tests via la façade OS stricte
        let output_result = os::exec_command_async("cargo", &["test", "--lib"], None).await;

        match output_result {
            Ok(_) => {
                // 2. Succès : Le code IA passe ses propres tests
                user_success!(
                    "MSG_CODEGEN_TESTS_SUCCESS",
                    json_value!({ "module": module_name })
                );
                Ok(())
            }
            Err(e) => {
                // 3. Échec : On extrait la sortie d'erreur (stdout/stderr capturé par l'OS)
                let raw_error = e.to_string();

                // L'astuce ici est de ne garder que le bloc pertinent pour l'IA (les paniques et assertions)
                // Cargo regroupe les erreurs à la fin sous la section "failures:"
                let feedback_payload = if let Some(idx) = raw_error.find("failures:") {
                    raw_error[idx..].trim().to_string()
                } else {
                    // Fallback si la structure est différente
                    raw_error.clone()
                };

                // STRICT : Notification IHM
                user_warn!(
                    "MSG_CODEGEN_TESTS_FAILED",
                    json_value!({ "module": module_name })
                );

                // STRICT : Levée de l'erreur AI-Ready pour déclencher le World Model
                raise_error!(
                "ERR_CODEGEN_TESTS_FAILED",
                error = "La Squad IA a généré un code qui ne passe pas les tests unitaires. Une correction logique est requise.",
                context = json_value!({
                    "module": module_name,
                    "xai_feedback": feedback_payload,
                    "status": "requires_ai_correction"
                })
            );
            }
        }
    }

    // =========================================================================
    // MODE DECOUVERTE
    // =========================================================================

    /// 📝 Helper : Transforme un chemin en slug (ex: "Cargo.toml" -> "cargo_toml")
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

    /// 📝 Helper : Transforme un nom technique en format lisible
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

    /// 📝 Helper : Génère un handle unique basé sur le chemin relatif
    fn generate_handle(&self, path: &Path, root: &Path, prefix: &str) -> String {
        let rel_path = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
        let slug = Self::slugify(&rel_path);
        format!("{}_{}", prefix, slug)
    }

    /// 📂 L'Agent Indexeur : Scanne un Crate Rust et le cartographie en (Services -> Components -> Modules)
    pub async fn index_workspace(
        &self,
        source_path: &Path,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<usize> {
        let mut count = 0;
        let jsonld_ctx = "db://_system/ontology/raise/@context/raise.jsonld";

        // 1. DÉTERMINER LA RACINE DU CRATE (Si on pointe sur src/, on remonte d'un cran)
        let root_dir = if source_path.ends_with("src") {
            source_path.parent().unwrap_or(source_path).to_path_buf()
        } else {
            source_path.to_path_buf()
        };
        let src_dir = root_dir.join("src");

        user_info!(
            "CODE_INDEX_START",
            json_value!({ "root": root_dir.to_string_lossy() })
        );

        // =====================================================================
        // ÉTAPE 1 : LE MACRO-SYSTÈME (ex: Service "raise" et son Cargo.toml)
        // =====================================================================
        let root_handle = root_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let root_service_id = format!("ref:services:handle:{}", root_handle);
        let root_name = Self::humanize(&root_handle);

        let root_service_doc = json_value!({
            "@context": jsonld_ctx,
            "@id": root_service_id,
            "@type": "Service",
            "handle": root_handle,
            "name": { "fr": root_name.clone(), "en": root_name },
            "version": "1.0.0",
            "status": "enabled",
        });
        manager
            .upsert_document("services", root_service_doc)
            .await?;
        count += 1;

        // Fichiers à la racine (Cargo.toml, README.md, etc.)
        if let Ok(mut entries) = fs::read_dir_async(&root_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_file() {
                    count += self
                        .upsert_module(
                            &path,
                            &root_dir,
                            &root_handle,
                            &root_service_id,
                            None,
                            manager,
                        )
                        .await?;
                }
            }
        }

        // Fichiers orphelins dans src/ (main.rs, lib.rs) appartiennent au macro-système
        if let Ok(mut entries) = fs::read_dir_async(&src_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_file() {
                    count += self
                        .upsert_module(
                            &path,
                            &root_dir,
                            &root_handle,
                            &root_service_id,
                            None,
                            manager,
                        )
                        .await?;
                }
            }
        }

        // =====================================================================
        // ÉTAPE 2 : LES SERVICES MÉTIER (ex: code_generator, ai, blockchain)
        // =====================================================================
        if fs::exists_async(&src_dir).await {
            let mut entries = fs::read_dir_async(&src_dir).await?;
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() {
                    let service_handle = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let service_id = format!("ref:services:handle:{}", service_handle);
                    let service_name = Self::humanize(&service_handle);

                    let service_doc = json_value!({
                        "@context": jsonld_ctx,
                        "@id": service_id,
                        "@type": "Service",
                        "handle": service_handle,
                        "name": { "fr": service_name.clone(), "en": service_name },
                        "version": "1.0.0",
                        "status": "enabled",
                    });
                    manager.upsert_document("services", service_doc).await?;
                    count += 1;

                    // =====================================================================
                    // ÉTAPE 3 : LES COMPOSANTS ET MODULES (Parcours récursif)
                    // =====================================================================
                    count += self
                        .index_directory_recursive(
                            &path,
                            &path,
                            &service_handle,
                            &service_id,
                            None,
                            manager,
                        )
                        .await?;
                }
            }
        }

        Ok(count)
    }

    /// 🔄 Parcours récursif pour générer les Components (Dossiers) et Modules (Fichiers)
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
                    // C'EST UN COMPOSANT (ex: "analyzers")
                    let comp_handle = self.generate_handle(&path, service_root, service_handle);
                    let comp_id = format!("ref:components:handle:{}", comp_handle);
                    let human_name =
                        Self::humanize(&path.file_name().unwrap_or_default().to_string_lossy());

                    let mut comp_doc = json_value!({
                        "@context": "db://_system/ontology/raise/@context/raise.jsonld",
                        "@id": comp_id,
                        "@type": "Component",
                        "handle": comp_handle,
                        "name": { "fr": human_name.clone(), "en": human_name },
                        "service_id": service_id,
                    });

                    if let Some(ref p_id) = parent_comp_id {
                        comp_doc["parent_id"] = json_value!(p_id);
                    }

                    manager.upsert_document("components", comp_doc).await?;
                    count += 1;

                    // Appel récursif pour les sous-composants
                    count += self
                        .index_directory_recursive(
                            &path,
                            service_root,
                            service_handle,
                            service_id,
                            Some(comp_id),
                            manager,
                        )
                        .await?;
                } else if path.is_file() {
                    // C'EST UN MODULE (ex: "semantic_analyzer.rs" ou "diff.rs")
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

    /// 📝 Helper pour uniformiser l'insertion des Modules (Fichiers)
    async fn upsert_module(
        &self,
        file_path: &Path,
        root_path: &Path,
        prefix_handle: &str,
        service_id: &str,
        component_id: Option<String>,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<usize> {
        let mod_handle = self.generate_handle(file_path, root_path, prefix_handle);
        let mod_id = format!("ref:modules:handle:{}", mod_handle);
        let human_name =
            Self::humanize(&file_path.file_stem().unwrap_or_default().to_string_lossy());

        let mut mod_doc = json_value!({
            "@context": "db://_system/ontology/raise/@context/raise.jsonld",
            "@id": mod_id,
            "@type": "Module",
            "handle": mod_handle,
            "name": { "fr": human_name.clone(), "en": human_name },
            "version": "1.0.0",
            "service_id": service_id,
        });

        // Si le fichier est dans un dossier, on le lie à son Component (Sinon, il est lié directement au Service)
        if let Some(c_id) = component_id {
            mod_doc["component_id"] = json_value!(c_id);
        }

        manager.upsert_document("modules", mod_doc).await?;
        Ok(1)
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_generator::models::{CodeElement, CodeElementType, Visibility};
    use crate::json_db::jsonld::VocabularyRegistry;
    use crate::utils::testing::DbSandbox;

    const TEST_SCHEMA: &str = "db://_system/_system/schemas/v1/db/generic.schema.json";

    #[async_test]
    async fn test_service_sync_flow_strict_ai_master() {
        // 1. Initialisation de l'environnement de test (Sandbox + Manager)
        let sandbox = DbSandbox::new().await;
        VocabularyRegistry::init_mock_for_tests();
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        manager
            .create_collection(
                "code_elements",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .expect("Échec de la création de la collection de test");

        let root = sandbox.storage.config.data_root.clone();
        let service = CodeGeneratorService::new(root.clone()).with_test_mode();
        let mut module = Module::new("ai_module", root.join("ai.rs")).unwrap();

        // 2. Construction de l'élément avec métadonnées IA et ACCOLADES
        module.elements.push(CodeElement {
            module_id: None,
            parent_id: None,
            attributes: vec!["#[ai_master]".to_string()],
            docs: Some("IA Documentation".to_string()),
            elements: vec![],
            handle: "fn:main".to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            signature: "pub fn main()".to_string(),
            body: Some("{ println!(\"Hello\"); }".to_string()),
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        });

        // 3. Phase Top-Down : Synchronisation physique (Weave -> Disk)
        service.sync_module(module.clone()).await.unwrap();

        // 4. Phase Bottom-Up : Ingestion (Disk -> Reconcile -> DB)
        // 🎯 FIX : Passage du manager requis par la signature
        service
            .ingest_file(&module.path, &manager, TEST_SCHEMA)
            .await
            .unwrap();

        // 5. Vérification de la restauration via une requête en base
        let query = Query::new("code_elements");
        let result = QueryEngine::new(&manager)
            .execute_query(query)
            .await
            .unwrap();

        assert_eq!(result.total_count, 1);
        let reconciled_doc = &result.documents[0];

        // 🕵️ Assertions sémantiques
        assert_eq!(reconciled_doc["handle"], "fn:main");

        let attrs = reconciled_doc["attributes"]
            .as_array()
            .expect("Attributs manquants");
        assert!(
            attrs.iter().any(|v| v == "#[ai_master]"),
            "Attribut #[ai_master] non restauré"
        );

        assert_eq!(
            reconciled_doc["docs"], "IA Documentation",
            "Documentation non restaurée"
        );
    }

    #[async_test]
    async fn test_service_ingest_file() {
        // 1. Initialisation de l'environnement de test
        let sandbox = DbSandbox::new().await;
        VocabularyRegistry::init_mock_for_tests();

        // On instancie le service et le manager DB
        let service =
            CodeGeneratorService::new(sandbox.storage.config.data_root.clone()).with_test_mode();
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        manager
            .create_collection(
                "code_elements",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .expect("Échec de la création de la collection de test");

        // 2. Création d'un faux fichier physique
        let file_path = sandbox.storage.config.data_root.join("test_ingest.rs");
        let rust_code = "
// @raise-handle: fn:test_ingest
pub fn test_ingest() {
    let a = 1;
}
";
        fs::write_async(&file_path, rust_code).await.unwrap();

        // 3. Exécution de l'Agent d'Ingestion
        let count = service
            .ingest_file(&file_path, &manager, TEST_SCHEMA)
            .await
            .expect("L'ingestion a échoué");
        assert_eq!(count, 1, "Un élément aurait dû être ingéré");

        // 4. Vérification en base de données
        let query = Query::new("code_elements");
        let result = QueryEngine::new(&manager)
            .execute_query(query)
            .await
            .unwrap();

        assert_eq!(result.total_count, 1);
        assert_eq!(result.documents[0]["handle"], "fn:test_ingest");
        assert_eq!(
            result.documents[0]["metadata"]["file_path"],
            file_path.to_string_lossy().to_string()
        );
    }

    #[async_test]
    async fn test_service_weave_file() {
        // 1. Initialisation
        let sandbox = DbSandbox::new().await;
        VocabularyRegistry::init_mock_for_tests();

        let service =
            CodeGeneratorService::new(sandbox.storage.config.data_root.clone()).with_test_mode();
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        manager
            .create_collection(
                "code_elements",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .expect("Échec de la création de la collection de test");

        // 2. Création du fichier et ingestion initiale
        let file_path = sandbox.storage.config.data_root.join("test_weave.rs");
        let rust_code = "
// @raise-handle: fn:test_weave
pub fn test_weave() {}
";
        fs::write_async(&file_path, rust_code).await.unwrap();
        service
            .ingest_file(&file_path, &manager, TEST_SCHEMA)
            .await
            .unwrap();

        // 3. Mutation par l'IA (Modification directe en base)
        let query = Query::new("code_elements");
        let result = QueryEngine::new(&manager)
            .execute_query(query)
            .await
            .unwrap();
        let mut doc = result.documents[0].clone();

        doc["body"] = json_value!("{ println!(\"AI was here\"); }");
        manager.upsert_document("code_elements", doc).await.unwrap();

        // 4. Exécution de l'Agent Forgeron (Weave)
        let final_path = service
            .weave_file("test_weave_mod", &file_path, &manager)
            .await
            .expect("Le tissage a échoué");

        // 5. Vérification Physique
        let final_code = fs::read_to_string_async(&final_path).await.unwrap();
        assert!(
            final_code.contains("AI was here"),
            "Le fichier n'a pas été mis à jour par la base de données !"
        );
    }
}
