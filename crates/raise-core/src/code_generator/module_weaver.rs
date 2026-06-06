use crate::code_generator::graph::sort_elements_topologically;
use crate::code_generator::models::{
    CodeElement, CodeElementType, ContractStatus, Module, StagedModule,
};
use crate::code_generator::weaver::Weavable;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Query, QueryEngine};
use crate::utils::prelude::*;

pub struct ModuleWeaver;

impl ModuleWeaver {
    /// 💾 Persiste le contrat de préparation (StagedModule) directement dans jsondb.
    /// Utilise le handle unique pour indexer le contrat sémantique de modification.
    pub async fn persist_stage(
        manager: &CollectionsManager<'_>,
        staged: &StagedModule,
        agent_handle: &str,
    ) -> RaiseResult<()> {
        let contract_handle = format!("stage_{}", staged.module_name);

        // 🛡️ 1. VÉRIFICATION DE CONCURRENCE (Le Verrou)
        let query = Query::new("staged_contracts");
        let db_result = match QueryEngine::new(manager).execute_query(query).await {
            Ok(res) => res,
            Err(e) => raise_error!("ERR_CODEGEN_QUERY_FAILED", error = e.to_string()),
        };

        for doc in db_result.documents {
            if doc.get("module_name").and_then(|v| v.as_str()) == Some(&staged.module_name)
                && doc.get("contract_status").and_then(|v| v.as_str()) == Some("pending")
            {
                let existing_agent = doc
                    .get("agent_handle")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                // Rejet strict : Le nœud d'architecture est déjà sous mutation
                raise_error!(
                    "ERR_CODEGEN_CONFLICT",
                    error = format!(
                        "Le module {} est déjà verrouillé par une mutation en cours.",
                        staged.module_name
                    ),
                    context = json_value!({
                        "module": staged.module_name,
                        "existing_agent": existing_agent,
                        "attempted_by": agent_handle
                    })
                );
            }
        }

        // 💾 2. CRÉATION DU CONTRAT (Si la voie est libre)
        let doc = json_value!({
            "handle": contract_handle,
            "@type": ["raise:StagedContract", "la:LogicalArchitectureUpdate"],
            "name": {
                "fr": format!("Contrat de préparation pour {}", staged.module_name),
                "en": format!("Staging contract for {}", staged.module_name)
            },
            "clearance": "C3-Privé",
            "module_name": staged.module_name.clone(),
            "agent_handle": agent_handle,
            "temp_path": staged.temp_path.to_string_lossy().to_string(),
            "final_path": staged.final_path.to_string_lossy().to_string(),
            "contract_status": "pending",
            "target_elements": json::serialize_to_value(&staged.target_elements).unwrap_or(json_value!([]))
        });

        match manager.upsert_document("staged_contracts", doc).await {
            Ok(_) => Ok(()),
            Err(e) => raise_error!(
                "ERR_PERSIST_STAGE",
                error = e.to_string(),
                context = json_value!({ "module": staged.module_name, "handle": contract_handle })
            ),
        }
    }

    /// 📤 Charge un contrat existant en statut "pending" depuis jsondb.
    /// Garantit le fail-fast si aucun contrat n'est actif pour ce module.
    pub async fn load_stage(
        manager: &CollectionsManager<'_>,
        module_name: &str,
    ) -> RaiseResult<StagedModule> {
        let query = Query::new("staged_contracts");
        let db_result = match QueryEngine::new(manager).execute_query(query).await {
            Ok(res) => res,
            Err(e) => raise_error!("ERR_CODEGEN_QUERY_FAILED", error = e.to_string()),
        };

        let mut found_doc = None;
        for doc in db_result.documents {
            if let Some(m_name) = doc.get("module_name").and_then(|v| v.as_str()) {
                if m_name == module_name
                    && doc.get("contract_status").and_then(|v| v.as_str()) == Some("pending")
                {
                    found_doc = Some(doc);
                    break;
                }
            }
        }

        let doc = match found_doc {
            Some(d) => d,
            None => {
                raise_error!(
                    "ERR_STAGE_NOT_FOUND",
                    context = json_value!({ "module": module_name })
                )
            }
        };

        let temp_path = PathBuf::from(
            doc.get("temp_path")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
        );
        let final_path = PathBuf::from(
            doc.get("final_path")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
        );
        let target_elements_val = doc
            .get("target_elements")
            .cloned()
            .unwrap_or(json_value!([]));

        let target_elements: Vec<CodeElement> =
            match json::deserialize_from_value(target_elements_val) {
                Ok(els) => els,
                Err(e) => {
                    raise_error!(
                        "ERR_DESERIALIZE_STAGE",
                        error = e.to_string(),
                        context = json_value!({ "module": module_name })
                    )
                }
            };

        let handle = doc
            .get("handle")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let agent_handle = doc
            .get("agent_handle")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        Ok(StagedModule {
            handle,
            agent_handle,
            contract_status: ContractStatus::Pending,
            temp_path,
            final_path,
            module_name: module_name.to_string(),
            target_elements,
        })
    }
    /// 🚀 Tisse un module complet. Structure mathématique pure : Tri -> Itération -> Concaténation.
    pub fn weave_to_string(module: &Module) -> RaiseResult<String> {
        let mut output = String::new();

        // 1. Bannière de Gouvernance
        output.push_str(
            "// =========================================================================\n",
        );
        output.push_str(&format!("// 🌌 RAISE GENERATED MODULE : {}\n", module.name));
        // 🎯 FIX : On retire module.id qui n'existe plus dans le struct Module
        output.push_str("// CE FICHIER EST SYNCHRONISÉ AVEC LE JUMEAU NUMÉRIQUE.\n");
        output.push_str(
            "// =========================================================================\n\n",
        );

        // 🆕 2. Partitionnement des éléments spatiaux (Haut, Milieu, Bas)
        let mut headers = Vec::new();
        let mut tests = Vec::new();
        let mut core_elements = Vec::new();

        for el in module.elements.clone() {
            match el.element_type {
                // 🎯 FIX : ModuleHeader est devenu ImportBlock dans l'ontologie
                CodeElementType::ImportBlock => headers.push(el),
                CodeElementType::TestModule => tests.push(el),
                _ => core_elements.push(el),
            }
        }

        // 3. Tri Topologique uniquement sur le cœur du code (Structs, Enums, Fns...)
        let sorted_core = match sort_elements_topologically(core_elements) {
            Ok(elements) => elements,
            Err(e) => raise_error!(
                "ERR_CODEGEN_SORT_FAILED",
                error = e.to_string(),
                context = json_value!({ "module": module.name })
            ),
        };

        // 4. Recomposition ordonnée : Headers -> Core -> Tests
        let mut final_sequence = headers;
        final_sequence.extend(sorted_core);
        final_sequence.extend(tests);

        // 5. Tissage Séquentiel
        for element in final_sequence {
            match element.weave() {
                Ok(element_code) => {
                    output.push_str(&element_code);
                    output.push_str("\n\n");
                }
                Err(e) => {
                    raise_error!(
                        "ERR_CODEGEN_ELEMENT_WEAVE_FAILED",
                        error = e.to_string(),
                        context = json_value!({
                            "module": module.name,
                            "handle": element.handle
                        })
                    )
                }
            }
        }

        Ok(output)
    }

    /// 💾 Persistance Physique
    pub async fn sync_to_disk(module: &Module, _root_path: &Path) -> RaiseResult<PathBuf> {
        let content = Self::weave_to_string(module)?;

        // Calcul du chemin
        let file_path = module.path.clone();

        // Assurer l'existence du dossier parent
        if let Some(parent) = file_path.parent() {
            fs::ensure_dir_async(parent).await?;
        }

        match fs::write_async(&module.path, content).await {
            Ok(_) => {
                user_success!(
                    "MSG_CODEGEN_SYNC_SUCCESS",
                    json_value!({ "module": module.name, "path": module.path.to_string_lossy() })
                );
            }
            Err(e) => raise_error!(
                "ERR_CODEGEN_DISK_IO_FAILED",
                error = e.to_string(),
                context = json_value!({ "path": module.path.to_string_lossy() })
            ),
        }

        Ok(file_path)
    }

    /// 🧪 Tisse le module et l'écrit dans le dossier de staging défini par le .env
    pub async fn weave_to_temp_file(module: &Module) -> RaiseResult<PathBuf> {
        let content = Self::weave_to_string(module)?;

        // 🎯 1. Utilisation exclusive de AppConfig et de la Façade OS
        let temp_dir = AppConfig::get()
            .get_path("PATH_TMP_FILE")
            .unwrap_or_else(|| os_temp_dir().join("raise_staging"));

        // 🎯 2. Assurer que le dossier cible existe (Façade FS)
        if let Err(e) = fs::ensure_dir_async(&temp_dir).await {
            raise_error!(
                "ERR_CODEGEN_TEMP_DIR",
                error = e.to_string(),
                context = json_value!({ "target_dir": temp_dir.to_string_lossy() })
            );
        }

        // 🎯 3. Génération d'un nom unique via l'horloge sémantique (Façade Core : UtcClock)
        let timestamp = UtcClock::now().timestamp_millis();
        let file_name = module
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let temp_path = temp_dir.join(format!("{}_{}", timestamp, file_name));

        // 🎯 4. Écriture physique asynchrone (Façade FS)
        match fs::write_async(&temp_path, content.as_bytes()).await {
            Ok(_) => Ok(temp_path),
            Err(e) => raise_error!(
                "ERR_CODEGEN_TEMP_WRITE_FAILED",
                error = e.to_string(),
                context = json_value!({ "temp_path": temp_path.to_string_lossy() })
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_generator::models::{CodeElement, Visibility};

    #[test]
    fn test_strict_module_weave_logic() {
        let mut module = Module::new("core_engine", PathBuf::from("engine.rs")).unwrap();

        let e1 = CodeElement {
            // 🎯 NOUVEAUX CHAMPS IA & TOPOLOGIE
            module_id: None,
            parent_id: None,
            attributes: vec![],
            docs: None,
            elements: vec![],
            // Champs classiques
            handle: "fn:main".to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            signature: "pub fn main()".to_string(),
            body: Some("{ run(); }".to_string()),
            dependencies: vec!["fn:run".to_string()],
            metadata: UnorderedMap::new(),
        };

        let e2 = CodeElement {
            module_id: None,
            parent_id: None,
            attributes: vec![],
            docs: None,
            elements: vec![],
            handle: "fn:run".to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Private,
            signature: "fn run()".to_string(),
            body: Some("{ println!(\"RAISE Active\"); }".to_string()),
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        };

        module.elements = vec![e1, e2];

        let result = ModuleWeaver::weave_to_string(&module).expect("Le tissage a échoué");
        assert!(result.contains("fn run()"));
        assert!(result.contains("pub fn main()"));
    }

    #[test]
    fn test_strict_spatial_ordering_for_ai() {
        let mut module = Module::new("test_mod", PathBuf::from("test.rs")).unwrap();

        module.elements.push(CodeElement {
            module_id: None,
            parent_id: None,
            attributes: vec![],
            docs: None,
            elements: vec![],
            handle: "sys:tests".to_string(),
            element_type: CodeElementType::TestModule,
            visibility: Visibility::Private,
            signature: "".to_string(),
            body: Some("#[cfg(test)]\nmod tests {}".to_string()),
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        });

        module.elements.push(CodeElement {
            module_id: None,
            parent_id: None,
            attributes: vec![],
            docs: None,
            elements: vec![],
            handle: "fn:logic".to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            signature: "fn logic()".to_string(),
            body: Some("{}".to_string()),
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        });

        module.elements.push(CodeElement {
            module_id: None,
            parent_id: None,
            attributes: vec![],
            docs: None,
            elements: vec![],
            handle: "sys:header".to_string(),
            element_type: CodeElementType::ImportBlock,
            visibility: Visibility::Private,
            signature: "".to_string(),
            body: Some("use UnorderedMap;".to_string()),
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        });

        let result = ModuleWeaver::weave_to_string(&module).unwrap();

        let header_pos = result.find("sys:header").unwrap();
        let logic_pos = result.find("fn:logic").unwrap();
        let tests_pos = result.find("sys:tests").unwrap();

        assert!(header_pos < logic_pos);
        assert!(logic_pos < tests_pos);
    }
}
