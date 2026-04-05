use crate::code_generator::graph::sort_elements_topologically;
use crate::code_generator::models::{CodeElementType, Module};
use crate::code_generator::weaver::Weavable;
use crate::utils::prelude::*;

pub struct ModuleWeaver;

impl ModuleWeaver {
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
