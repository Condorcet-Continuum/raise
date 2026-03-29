use crate::code_generator::models::{CodeElement, CodeElementType, Visibility};
use crate::utils::prelude::*;

/// 🛠️ Trait de tissage manuel (Zéro dépendance lourde)
pub trait Weavable {
    fn weave(&self) -> RaiseResult<String>;
}

impl Weavable for CodeElement {
    fn weave(&self) -> RaiseResult<String> {
        let mut buffer = String::new();

        buffer.push_str(&format!("// @raise-handle: {}\n", self.handle));

        // 1. Gestion manuelle de la visibilité
        let vis_str = match &self.visibility {
            Visibility::Public => "pub ",
            Visibility::Crate => "pub(crate) ",
            Visibility::Private => "",
            Visibility::Restricted(path) => {
                if path.is_empty() {
                    raise_error!(
                        "ERR_CODEGEN_INVALID_VISIBILITY",
                        error = "Le chemin de restriction est vide"
                    );
                }
                // Utilisation de format! au lieu de quote!
                &format!("pub({}) ", path)
            }
        };

        // 2. Construction du bloc de code selon le type
        // On se base sur la signature fournie par l'IA et on y injecte le corps.
        match self.element_type {
            CodeElementType::Function => {
                let body = self.body.as_deref().unwrap_or("{}");
                buffer.push_str(&format!("{}{}\n{}", vis_str, self.signature, body));
            }
            CodeElementType::Struct | CodeElementType::Enum | CodeElementType::Trait => {
                buffer.push_str(&format!("{}{}", vis_str, self.signature));
            }
            CodeElementType::Impl => {
                let body = self.body.as_deref().unwrap_or(" {}");
                buffer.push_str(&format!("{} {}", self.signature, body));
            }
            // 🆕 Tissage spécifique pour l'IA
            CodeElementType::ModuleHeader | CodeElementType::TestModule => {
                // Pas de modificateur de visibilité, on injecte directement le code brut (ex: les `use`)
                let body = self.body.as_deref().unwrap_or("");
                buffer.push_str(&format!("{}\n", body));
            }
            _ => {
                buffer.push_str(&format!("{}{}", vis_str, self.signature));
            }
        }

        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_generator::models::CodeElementType;

    #[test]
    fn test_manual_weave_function() {
        let element = CodeElement {
            handle: "fn:calculate".to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            signature: "fn calculate(val: u32) -> u32".to_string(),
            body: Some("{\n    val * 2\n}".to_string()),
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        };

        let result = element.weave().expect("Le tissage manuel a échoué");

        // 🎯 FIX : Le test doit maintenant s'attendre à l'ancre en premier
        assert!(result.contains("// @raise-handle: fn:calculate"));
        assert!(result.contains("pub fn calculate"));
        assert!(result.contains("val * 2"));
    }

    #[test]
    fn test_manual_weave_private_visibility() {
        let element = CodeElement {
            handle: "struct:internal".to_string(),
            element_type: CodeElementType::Struct,
            visibility: Visibility::Private,
            signature: "struct Internal;".to_string(),
            body: None,
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        };

        let result = element.weave().unwrap();

        // 🎯 FIX : On vérifie l'ancre ET l'absence de 'pub'
        let expected = "// @raise-handle: struct:internal\nstruct Internal;";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_error_on_invalid_restricted_visibility() {
        let element = CodeElement {
            handle: "fn:broken".to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Restricted("".to_string()),
            signature: "fn broken()".to_string(),
            body: None,
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        };

        let result = element.weave();
        assert!(result.is_err());

        if let Err(AppError::Structured(data)) = result {
            assert_eq!(data.code, "ERR_CODEGEN_INVALID_VISIBILITY");
        }
    }
}
