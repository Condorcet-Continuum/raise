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
        output.push_str(&format!("// ID : {}\n", module.id));
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
                CodeElementType::ModuleHeader => headers.push(el),
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
    pub fn sync_to_disk(module: &Module) -> RaiseResult<()> {
        let content = Self::weave_to_string(module)?;

        // 🎯 FIX : Assurer l'existence du dossier parent (ex: src/)
        if let Some(parent) = module.path.parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all_sync(parent) {
                    raise_error!(
                        "ERR_CODEGEN_DIR_CREATION_FAILED",
                        error = e.to_string(),
                        context = json_value!({ "path": parent.to_string_lossy() })
                    );
                }
            }
        }

        // Écriture finale via la façade FS
        match fs::write_sync(&module.path, content) {
            Ok(_) => {
                user_success!(
                    "MSG_CODEGEN_SYNC_SUCCESS",
                    json_value!({ "module": module.name, "path": module.path.to_string_lossy() })
                );
                Ok(())
            }
            Err(e) => raise_error!(
                "ERR_CODEGEN_DISK_IO_FAILED",
                error = e.to_string(),
                context = json_value!({ "path": module.path.to_string_lossy() })
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_generator::models::{CodeElement, CodeElementType, Visibility};

    #[test]
    fn test_strict_module_weave_logic() {
        let mut module = Module::new("core_engine", PathBuf::from("engine.rs")).unwrap();

        let e1 = CodeElement {
            handle: "fn:main".to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            signature: "fn main()".to_string(),
            body: Some("{ run(); }".to_string()),
            dependencies: vec!["fn:run".to_string()],
            metadata: UnorderedMap::new(),
        };

        let e2 = CodeElement {
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

        // Vérification de l'ordre mathématique (dépendance d'abord)
        assert!(result.find("fn run()").unwrap() < result.find("pub fn main()").unwrap());
        assert!(result.contains("🌌 RAISE GENERATED MODULE"));
    }

    #[test]
    fn test_weave_error_propagation_with_raise_error() {
        let mut module = Module::new("faulty_mod", PathBuf::from("faulty.rs")).unwrap();

        // On force une erreur de visibilité
        let bad_element = CodeElement {
            handle: "fn:error".to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Restricted("".to_string()),
            signature: "fn error()".to_string(),
            body: None,
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        };

        module.elements = vec![bad_element];

        let result = ModuleWeaver::weave_to_string(&module);

        assert!(result.is_err());
        match result {
            Err(AppError::Structured(data)) => {
                assert_eq!(data.code, "ERR_CODEGEN_ELEMENT_WEAVE_FAILED");
                assert_eq!(data.context["handle"], "fn:error");
            }
            _ => panic!("L'erreur devrait être structurée via raise_error!"),
        }
    }

    #[test]
    fn test_strict_spatial_ordering_for_ai() {
        let mut module = Module::new("test_mod", PathBuf::from("test.rs")).unwrap();

        // On insère volontairement dans le désordre pour tester
        module.elements.push(CodeElement {
            handle: "sys:tests".to_string(),
            element_type: CodeElementType::TestModule,
            visibility: Visibility::Private,
            signature: "".to_string(),
            body: Some("#[cfg(test)]\nmod tests {}".to_string()),
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        });

        module.elements.push(CodeElement {
            handle: "fn:logic".to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            signature: "fn logic()".to_string(),
            body: Some("{}".to_string()),
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        });

        module.elements.push(CodeElement {
            handle: "sys:header".to_string(),
            element_type: CodeElementType::ModuleHeader,
            visibility: Visibility::Private,
            signature: "".to_string(),
            body: Some("use std::collections::HashMap;".to_string()),
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        });

        let result = ModuleWeaver::weave_to_string(&module).unwrap();

        // Vérification des positions
        let header_pos = result.find("sys:header").unwrap();
        let logic_pos = result.find("fn:logic").unwrap();
        let tests_pos = result.find("sys:tests").unwrap();

        assert!(
            header_pos < logic_pos,
            "Le Header doit être avant le code logique"
        );
        assert!(
            logic_pos < tests_pos,
            "Le code logique doit être avant les tests"
        );
    }
}
