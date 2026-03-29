// =========================================================================
//  RAISE CODE GENERATOR : AST Weaver Root Façade (V2 Strict)
// =========================================================================

pub mod analyzers; // Analyse sémantique Arcadia
pub mod diff; // Moteur de comparaison (Jumeau vs Physique)
pub mod graph; // Tri topologique des dépendances
pub mod models; // Modèles de données (CodeElement, Module)
pub mod module_weaver; // Orchestration du tissage fichier
pub mod reconciler; // Extraction Bottom-Up via @raise-handle
pub mod utils; // Utilitaires mathématiques (String transformation)
pub mod weaver; // Tissage unitaire des blocs de code

use self::diff::{DiffAction, DiffEngine};
use self::models::Module;
use self::module_weaver::ModuleWeaver;
use self::reconciler::Reconciler;
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
    ) -> RaiseResult<usize> {
        if !path.exists() {
            raise_error!(
                "ERR_CODEGEN_FILE_NOT_FOUND",
                error = "Le fichier source n'existe pas physiquement.",
                context = json_value!({ "path": path.to_string_lossy() })
            );
        }

        // 1. Extraction Lexicale
        let elements = Reconciler::parse_from_file(path)?;

        // 2. Préparation de la collection
        let _ = manager
            .create_collection(
                "code_elements",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await;

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
            if let Err(e) = self.format_module(&full_path) {
                user_info!(
                    "MSG_CODEGEN_PRE_SYNC_FMT_FAILED",
                    json_value!({ "path": full_path.to_string_lossy(), "error": e.to_string() })
                );
                // Si rustfmt échoue, c'est que l'Agent a généré du code syntaxiquement invalide.
                // On bloque la synchronisation pour protéger le Jumeau Numérique.
                return Err(e);
            }

            let physical_elements = match Reconciler::parse_from_file(&full_path) {
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
            if let Err(e) = fs::copy_sync(&full_path, &backup_path) {
                raise_error!(
                    "ERR_CODEGEN_BACKUP_FAILED",
                    error = e.to_string(),
                    context = json_value!({ "path": full_path.to_string_lossy() })
                );
            }
        }

        // 4.2 Tissage Top-Down
        if let Err(e) = ModuleWeaver::sync_to_disk(&module) {
            Self::rollback(&full_path, &backup_path, file_exists);
            return Err(e); // L'erreur de tissage remonte
        }

        // 4.3 Formatage de propreté
        let _ = self.format_module(&full_path);

        // 4.4 Compilation stricte (cargo check)
        if let Err(e) = self.check_workspace(&module.name) {
            Self::rollback(&full_path, &backup_path, file_exists);
            return Err(e); // Propage l'erreur structurée (avec logs cargo) à l'IA
        }

        // 4.5 Exécution des tests (cargo test)
        if let Err(e) = self.test_workspace(&module.name) {
            Self::rollback(&full_path, &backup_path, file_exists);
            return Err(e); // Propage l'erreur structurée (avec logs de test) à l'IA
        }

        // 4.6 Validation de la transaction (Commit)
        if file_exists {
            let _ = fs::remove_file_sync(&backup_path);
        }

        Ok(full_path)
    }

    /// Met à jour ou insère un élément dans la liste du module.
    /// non utilisé pour l'instant
    /*
    fn update_element_in_module(&self, module: &mut Module, new_element: models::CodeElement) {
        let mut found = false;
        for el in &mut module.elements {
            if el.handle == new_element.handle {
                *el = new_element.clone();
                found = true;
                break;
            }
        }
        if !found {
            module.elements.push(new_element);
        }
    }
    */
    /// 🧹 Post-process : Formatage du code (Anciennement lié à Clippy).
    pub fn format_module(&self, path: &Path) -> RaiseResult<()> {
        match os::exec_command_sync("rustfmt", &[path.to_string_lossy().as_ref()], None) {
            Ok(_) => Ok(()),
            Err(e) => raise_error!(
                "ERR_CODEGEN_FMT_FAILED",
                error = e.to_string(),
                context = json_value!({ "path": path.to_string_lossy() })
            ),
        }
    }
    /// ⏪ Restaure le fichier dans son état précédent en cas d'échec de l'IA
    fn rollback(target: &Path, backup: &Path, existed_before: bool) {
        if existed_before {
            let _ = fs::copy_sync(backup, target);
            let _ = fs::remove_file_sync(backup);
        } else {
            // Si le fichier n'existait pas du tout avant, on le supprime simplement
            let _ = fs::remove_file_sync(target);
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

    /// ⚖️ Le Juge de Paix : Vérifie que le projet compile
    fn check_workspace(&self, module_name: &str) -> RaiseResult<()> {
        // 🛑 COURT-CIRCUIT : On ne lance pas de sous-compilation pendant les tests unitaires
        if cfg!(test) || self.skip_compilation {
            return Ok(());
        }
        // On peut ajouter --message-format=json pour que l'Agent IA parse facilement l'erreur
        match os::exec_command_sync("cargo", &["check", "--lib", "--message-format=json"], None) {
            Ok(_) => Ok(()),
            Err(e) => raise_error!(
                "ERR_CODEGEN_COMPILATION_FAILED",
                error = "L'Agent IA a généré du code qui ne compile pas.",
                context = json_value!({
                    "module": module_name,
                    "compiler_output": e.to_string()
                })
            ),
        }
    }

    /// 🔥 L'Épreuve du Feu : Exécute les tests unitaires
    fn test_workspace(&self, module_name: &str) -> RaiseResult<()> {
        // 🛑 COURT-CIRCUIT : Évite une boucle infinie (test qui lance des tests)
        if cfg!(test) || self.skip_compilation {
            return Ok(());
        }

        match os::exec_command_sync("cargo", &["test", "--lib"], None) {
            Ok(_) => Ok(()),
            Err(e) => raise_error!(
                "ERR_CODEGEN_TESTS_FAILED",
                error = "Les tests unitaires générés par l'Agent ont échoué.",
                context = json_value!({
                    "module": module_name,
                    "test_output": e.to_string()
                })
            ),
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
        if let Ok(entries) = fs::read_dir_sync(&root_dir) {
            for entry in entries.flatten() {
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
        if let Ok(entries) = fs::read_dir_sync(&src_dir) {
            for entry in entries.flatten() {
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
        if fs::exists_sync(&src_dir) {
            let entries = fs::read_dir_sync(&src_dir)?;
            for entry in entries.flatten() {
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
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = RaiseResult<usize>> + Send + 'a>> {
        Box::pin(async move {
            let mut count = 0;
            let entries = fs::read_dir_sync(current_dir)?;

            for entry in entries.flatten() {
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

    #[async_test]
    async fn test_service_sync_flow_strict_ai_master() {
        let dir = tempdir().unwrap();
        // 🎯 FIX : On n'oublie pas d'activer le mode test pour court-circuiter cargo !
        let service = CodeGeneratorService::new(dir.path().to_path_buf()).with_test_mode();

        let mut module = Module::new("test_mod", PathBuf::from("test_mod.rs")).unwrap();
        module.elements.push(CodeElement {
            handle: "fn:main".to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            signature: "fn main()".to_string(),
            body: Some("{ println!(\"AI Power\"); }".to_string()),
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        });

        // Premier passage : Création du fichier par l'IA
        let path = service
            .sync_module(module.clone())
            .await
            .expect("Sync initial échoué");
        assert!(path.exists());

        // Simulation d'une tentative de modification par un humain (ou un processus externe)
        let modified_content = "
// @raise-handle: fn:main
pub fn main() {
    println!(\"Human interference\");
}
";
        fs::write_sync(&path, modified_content).unwrap();

        // Second passage : Le Jumeau Numérique doit écraser l'interférence
        let _ = service.sync_module(module).await.expect("Re-sync échoué");

        let final_content = fs::read_to_string_sync(&path).unwrap();

        // 🎯 NOUVELLES ASSERTIONS : L'IA a repris le contrôle
        assert!(
            !final_content.contains("Human interference"),
            "Le système aurait dû écraser l'interférence humaine !"
        );
        assert!(
            final_content.contains("AI Power"),
            "Le système n'a pas restauré l'état de l'IA."
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

        // 2. Création d'un faux fichier physique
        let file_path = sandbox.storage.config.data_root.join("test_ingest.rs");
        let rust_code = "
// @raise-handle: fn:test_ingest
pub fn test_ingest() {
    let a = 1;
}
";
        fs::write_sync(&file_path, rust_code).unwrap();

        // 3. Exécution de l'Agent d'Ingestion
        let count = service
            .ingest_file(&file_path, &manager)
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

        // 2. Création du fichier et ingestion initiale
        let file_path = sandbox.storage.config.data_root.join("test_weave.rs");
        let rust_code = "
// @raise-handle: fn:test_weave
pub fn test_weave() {}
";
        fs::write_sync(&file_path, rust_code).unwrap();
        service.ingest_file(&file_path, &manager).await.unwrap();

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
        let final_code = fs::read_to_string_sync(&final_path).unwrap();
        assert!(
            final_code.contains("AI was here"),
            "Le fichier n'a pas été mis à jour par la base de données !"
        );
    }
}
