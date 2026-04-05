use crate::code_generator::models::CodeElement;
use crate::utils::prelude::*;

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub enum DiffAction {
    Upsert, // Création ou mise à jour nécessaire
    Ignore, // Identique
    Notify, // Changement détecté nécessitant une validation IA/Humaine
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct DiffReport {
    pub handle: String,
    pub action: DiffAction,
    pub reason: String,
}

pub struct DiffEngine;

impl DiffEngine {
    /// 🧹 Normalise une chaîne en supprimant tous les espaces blancs pour une comparaison stricte.
    fn normalize_code(code: Option<&String>) -> String {
        match code {
            Some(c) => {
                let mut result = String::with_capacity(c.len());
                let mut in_string = false;
                let mut prev_char = '\0';

                for ch in c.chars() {
                    // On détecte l'entrée/sortie d'une chaîne de caractères
                    if ch == '"' && prev_char != '\\' {
                        in_string = !in_string;
                    }

                    // On conserve le caractère si on est dans une chaîne, ou si ce n'est pas un espace
                    if in_string || !ch.is_whitespace() {
                        result.push(ch);
                    }
                    prev_char = ch;
                }
                result
            }
            None => String::new(),
        }
    }
    /// ⚖️ Compare les éléments du fichier avec ceux de la base de données.
    /// Algorithme : O(N) via indexation sémantique.
    pub fn compute_diff(
        from_file: Vec<CodeElement>,
        from_db: Vec<CodeElement>,
    ) -> RaiseResult<Vec<DiffReport>> {
        let mut reports = Vec::new();

        let mut db_map = UnorderedMap::new();
        for el in from_db {
            db_map.insert(el.handle.clone(), el);
        }

        for file_el in from_file {
            match db_map.get(&file_el.handle) {
                Some(db_el) => {
                    let mut has_changed = false;
                    let mut reasons = Vec::new();

                    // 🎯 FIX : Comparaison canonisée ignorant les espaces
                    if Self::normalize_code(file_el.body.as_ref())
                        != Self::normalize_code(db_el.body.as_ref())
                    {
                        has_changed = true;
                        reasons.push("BODY_MODIFIED");
                    }
                    if Self::normalize_code(Some(&file_el.signature))
                        != Self::normalize_code(Some(&db_el.signature))
                    {
                        has_changed = true;
                        reasons.push("SIGNATURE_MODIFIED");
                    }
                    if file_el.visibility != db_el.visibility {
                        has_changed = true;
                        reasons.push("VISIBILITY_MODIFIED");
                    }

                    if has_changed {
                        reports.push(DiffReport {
                            handle: file_el.handle.clone(),
                            action: DiffAction::Upsert,
                            reason: reasons.join("|"),
                        });
                    }
                }
                None => {
                    reports.push(DiffReport {
                        handle: file_el.handle.clone(),
                        action: DiffAction::Upsert,
                        reason: "NEW_ELEMENT_FOUND".to_string(),
                    });
                }
            }
        }

        Ok(reports)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_generator::models::{CodeElementType, Visibility};

    fn mock_el(handle: &str, body: &str) -> CodeElement {
        CodeElement {
            // 🎯 NOUVEAUX CHAMPS (Initialisation par défaut pour le mock)
            module_id: None,
            parent_id: None,
            attributes: vec![],
            docs: None,
            elements: vec![],

            // Champs existants
            handle: handle.to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            signature: format!("fn {}()", handle),
            body: Some(body.to_string()),
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        }
    }

    #[test]
    fn test_diff_detection_body_changed() {
        let db_state = vec![mock_el("fn:sync", "{ old_logic(); }")];
        let file_state = vec![mock_el("fn:sync", "{ new_logic(); }")];

        let results = DiffEngine::compute_diff(file_state, db_state).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, DiffAction::Upsert);
        assert!(results[0].reason.contains("BODY_MODIFIED"));
    }

    #[test]
    fn test_diff_ignore_identical() {
        let db_state = vec![mock_el("fn:stable", "{}")];
        let file_state = vec![mock_el("fn:stable", "{}")];

        let results = DiffEngine::compute_diff(file_state, db_state).unwrap();

        assert_eq!(results.len(), 0, "Aucun diff ne devrait être généré");
    }

    #[test]
    fn test_diff_new_element_added_by_human() {
        let db_state = vec![];
        let file_state = vec![mock_el("fn:human_addition", "{}")];

        let results = DiffEngine::compute_diff(file_state, db_state).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].reason, "NEW_ELEMENT_FOUND");
    }

    #[test]
    fn test_diff_ignore_formatting() {
        // Le Jumeau Numérique a une version compacte
        let db_state = vec![mock_el("fn:format", "{ let x=1; }")];

        // Le fichier physique a été formaté par rustfmt avec des espaces et des sauts de ligne
        let file_state = vec![mock_el("fn:format", "{\n    let x = 1;\n}")];

        let results = DiffEngine::compute_diff(file_state, db_state).unwrap();

        // Le DiffEngine doit ignorer ces changements purement cosmétiques
        assert_eq!(
            results.len(),
            0,
            "Les différences de formatage doivent être ignorées"
        );
    }
}
