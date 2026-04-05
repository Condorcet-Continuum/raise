use crate::code_generator::models::{CodeElement, CodeElementType, Visibility};
use crate::utils::prelude::*;

/// 🛠️ Trait de tissage manuel (Zéro dépendance lourde)
pub trait Weavable {
    fn weave(&self) -> RaiseResult<String>;
}

impl Weavable for CodeElement {
    fn weave(&self) -> RaiseResult<String> {
        let mut buffer = String::new();

        // Marqueur de gouvernance RAISE
        buffer.push_str(&format!("// @raise-handle: {}\n", self.handle));

        // 1. Documentation (///)
        if let Some(ref d) = self.docs {
            for line in d.lines() {
                buffer.push_str(&format!("/// {}\n", line));
            }
        }

        // 2. Attributs (#[...])
        for attr in &self.attributes {
            buffer.push_str(&format!("{}\n", attr));
        }

        // 4. Gestion de la visibilité
        let vis_str = match &self.visibility {
            Visibility::Public => "pub ",
            Visibility::Crate => "pub(crate) ",
            Visibility::Protected => "/* protected */ ", // Sémantique MBSE, commentaire en Rust
            Visibility::Private => "",
        };

        // 5. Construction du bloc de code selon le type
        match self.element_type {
            CodeElementType::Function | CodeElementType::TestFunction => {
                let body = self.body.as_deref().unwrap_or("{}");
                buffer.push_str(&format!("{}{}\n{}", vis_str, self.signature, body));
            }
            CodeElementType::ImplBlock => {
                let body = self.body.as_deref().unwrap_or(" {}");
                buffer.push_str(&format!("{} {}", self.signature, body));
            }
            CodeElementType::ImportBlock | CodeElementType::TestModule => {
                // Pas de modificateur de visibilité, on injecte le contenu brut (ex: les `use`)
                let body = self.body.as_deref().unwrap_or("");
                buffer.push_str(&format!("{}\n", body));
            }
            _ => {
                // Struct, Enum, Trait, Constant, etc.
                buffer.push_str(&format!("{}{}", vis_str, self.signature));
            }
        }

        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weave_with_full_metadata() {
        let element = CodeElement {
            module_id: None,
            parent_id: None,
            elements: vec![],
            handle: "fn:calculate".to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            docs: Some("Calcule le double.\nSupporte l'IA.".to_string()),
            attributes: vec!["#[inline]".to_string()],
            signature: "fn calculate(val: u32) -> u32".to_string(),
            body: Some("{\n    val * 2\n}".to_string()),
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        };

        let result = element.weave().expect("Le tissage a échoué");

        // Vérification de la reconstruction ordonnée
        assert!(result.contains("/// Calcule le double."));
        assert!(result.contains("#[inline]"));
        assert!(result.contains("// @raise-handle: fn:calculate"));
        assert!(result.contains("pub fn calculate"));
    }

    #[test]
    fn test_manual_weave_private_visibility() {
        let element = CodeElement {
            module_id: None,
            parent_id: None,
            attributes: vec![],
            docs: None,
            elements: vec![],
            handle: "struct:internal".to_string(),
            element_type: CodeElementType::Struct,
            visibility: Visibility::Private,
            signature: "struct Internal;".to_string(),
            body: None,
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        };

        let result = element.weave().unwrap();
        assert!(result.contains("// @raise-handle: struct:internal"));
        assert!(!result.contains("pub "));
    }
}
