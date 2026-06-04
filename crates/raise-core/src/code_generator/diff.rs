// FICHIER : src-tauri/src/code_generator/diff.rs

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
    /// 🧠 Canonise le code pour une comparaison sémantique.
    /// Un mini-lexer qui ignore les espaces superflus et retire les commentaires,
    /// tout en préservant l'espacement vital entre les mots-clés (ex: "pub fn").
    fn canonicalize_code(code: Option<&String>) -> String {
        let text = match code {
            Some(c) => c,
            None => return String::new(),
        };

        let mut result = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();

        let mut in_string = false;
        let mut in_line_comment = false;
        let mut in_block_comment = false;
        let mut prev_char = '\0';
        let mut last_pushed_is_space = false;

        while let Some(ch) = chars.next() {
            // 1. Gestion des contextes (Commentaires et Strings)
            if in_line_comment {
                if ch == '\n' {
                    in_line_comment = false;
                }
                continue;
            }
            if in_block_comment {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next(); // On consomme le '/'
                    in_block_comment = false;
                }
                continue;
            }
            if !in_string {
                if ch == '/' && chars.peek() == Some(&'/') {
                    in_line_comment = true;
                    chars.next(); // On consomme le second '/'
                    continue;
                }
                if ch == '/' && chars.peek() == Some(&'*') {
                    in_block_comment = true;
                    chars.next(); // On consomme l'étoile
                    continue;
                }
            }

            // Bascule de chaîne (avec gestion naïve de l'échappement)
            if ch == '"' && prev_char != '\\' {
                in_string = !in_string;
            }

            // 2. Traitement du flux utile
            if in_string {
                result.push(ch);
                last_pushed_is_space = false;
            } else if ch.is_whitespace() {
                // Compression de tous les espaces/retours chariots en un seul
                if !last_pushed_is_space && !result.is_empty() {
                    result.push(' ');
                    last_pushed_is_space = true;
                }
            } else {
                // Si on tombe sur un symbole structurel, on supprime l'espace qui le précède
                let is_symbol = "{}()[]:;,.=+-*/<>!&|".contains(ch);
                if last_pushed_is_space && is_symbol {
                    result.pop();
                }
                result.push(ch);

                // Astuce : si c'est un symbole, on fait "comme si" on avait mis un espace
                // pour que le prochain espace réel soit ignoré par le bloc au-dessus.
                last_pushed_is_space = is_symbol;
            }
            prev_char = ch;
        }

        result.trim().to_string()
    }

    /// ⚖️ Compare les éléments du fichier avec ceux de la base de données.
    /// Algorithme : O(N) via indexation sémantique et canonisation lexicale.
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

                    // 🎯 Utilisation du nouveau moteur de canonisation
                    if Self::canonicalize_code(file_el.body.as_ref())
                        != Self::canonicalize_code(db_el.body.as_ref())
                    {
                        has_changed = true;
                        reasons.push("BODY_MODIFIED");
                    }
                    if Self::canonicalize_code(Some(&file_el.signature))
                        != Self::canonicalize_code(Some(&db_el.signature))
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
            module_id: None,
            parent_id: None,
            attributes: vec![],
            docs: None,
            elements: vec![],
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
        let db_state = vec![mock_el("fn:format", "{ let x=1; }")];
        let file_state = vec![mock_el("fn:format", "{\n    let x = 1;\n}")];

        let results = DiffEngine::compute_diff(file_state, db_state).unwrap();
        assert_eq!(
            results.len(),
            0,
            "Les différences de formatage doivent être ignorées"
        );
    }

    #[test]
    fn test_diff_ignore_comments_and_preserve_keywords() {
        // Le Jumeau a le code brut
        let db_state = vec![mock_el("fn:logic", "{ let mut active = true; }")];

        // Le fichier a été documenté par un humain
        let file_state = vec![mock_el(
            "fn:logic",
            "{\n    // Activation du système\n    let mut active = true; /* TODO: refactor */ \n}",
        )];

        let results = DiffEngine::compute_diff(file_state, db_state).unwrap();

        // Le DiffEngine doit ignorer les commentaires sans fusionner "let mut" en "letmut"
        assert_eq!(
            results.len(),
            0,
            "Les commentaires injectés ne doivent pas déclencher un faux positif"
        );
    }
}
