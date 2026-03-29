use crate::code_generator::models::{CodeElement, CodeElementType, Visibility};
use crate::utils::prelude::*;

#[derive(Debug, PartialEq)]
enum LexerState {
    Normal,
    InString,
    InChar,
    InLineComment,
    InBlockComment(usize), // Gère les commentaires imbriqués /* /* */ */
}

pub struct Reconciler;

impl Reconciler {
    pub fn parse_from_file(path: &Path) -> RaiseResult<Vec<CodeElement>> {
        let content = match fs::read_to_string_sync(path) {
            Ok(c) => c,
            Err(e) => raise_error!("ERR_RECONCILER_READ_FAILED", error = e.to_string()),
        };

        let mut elements = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();
            if line.starts_with("// @raise-handle:") {
                let handle = line.replace("// @raise-handle:", "").trim().to_string();
                let (element, next_index) = Self::extract_element_at(&handle, &lines, i + 1)?;
                elements.push(element);
                i = next_index;
                continue;
            }
            i += 1;
        }
        Ok(elements)
    }

    fn extract_element_at(
        handle: &str,
        lines: &[&str],
        start_index: usize,
    ) -> RaiseResult<(CodeElement, usize)> {
        let mut sig_idx = start_index;
        while sig_idx < lines.len() && lines[sig_idx].trim().is_empty() {
            sig_idx += 1;
        }

        if sig_idx >= lines.len() {
            raise_error!(
                "ERR_RECONCILER_EOF",
                context = json_value!({ "handle": handle })
            );
        }

        let signature_line = lines[sig_idx].trim();

        // --- MACHINE À ÉTATS LEXICALE ---
        let mut state = LexerState::Normal;
        let mut brace_count = 0;
        let mut in_body = false;
        let mut body = String::new();
        let mut current_index = sig_idx + 1; // On commence à stocker le body après la signature

        // Scan initial de la ligne de signature pour initialiser in_body et brace_count
        Self::scan_line_for_braces(signature_line, &mut state, &mut brace_count, &mut in_body);

        // 🎯  On réinitialise l'état si la ligne de signature était un commentaire
        if state == LexerState::InLineComment {
            state = LexerState::Normal;
        }

        // Scan du reste du corps
        if !in_body || brace_count > 0 {
            while current_index < lines.len() {
                let line = lines[current_index];
                body.push_str(line);
                body.push('\n');

                Self::scan_line_for_braces(line, &mut state, &mut brace_count, &mut in_body);

                // Remise à zéro du commentaire de ligne à la fin de chaque ligne
                if state == LexerState::InLineComment {
                    state = LexerState::Normal;
                }

                if in_body && brace_count == 0 {
                    break;
                }
                current_index += 1;
            }
        }

        // 🎯 Validation de clôture mathématique stricte
        if !in_body || brace_count != 0 {
            raise_error!(
                "ERR_RECONCILER_UNBALANCED_BRACES",
                context = json_value!({ "handle": handle })
            );
        }

        let element = CodeElement {
            handle: handle.to_string(),
            element_type: CodeElementType::Function,
            visibility: if signature_line.starts_with("pub ") {
                Visibility::Public
            } else {
                Visibility::Private
            },
            signature: signature_line.to_string(),
            body: Some(body.trim_end().to_string()),
            dependencies: Vec::new(),
            metadata: UnorderedMap::new(),
        };

        Ok((element, current_index + 1))
    }

    /// Fonction utilitaire pure gérant les transitions d'état du Lexer
    fn scan_line_for_braces(
        line: &str,
        state: &mut LexerState,
        brace_count: &mut isize,
        in_body: &mut bool,
    ) {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];
            let next_c = chars.get(i + 1).copied();

            match state {
                LexerState::Normal => match (c, next_c) {
                    ('/', Some('/')) => {
                        *state = LexerState::InLineComment;
                        break; // On ignore le reste de la ligne
                    }
                    ('/', Some('*')) => {
                        *state = LexerState::InBlockComment(1);
                        i += 1;
                    }
                    ('"', _) => *state = LexerState::InString,
                    ('\'', _) => *state = LexerState::InChar,
                    ('{', _) => {
                        *brace_count += 1;
                        *in_body = true;
                    }
                    ('}', _) => {
                        *brace_count -= 1;
                    }
                    _ => {}
                },
                LexerState::InString => {
                    if c == '\\' {
                        i += 1; // Ignore le caractère échappé (ex: \")
                    } else if c == '"' {
                        *state = LexerState::Normal;
                    }
                }
                LexerState::InChar => {
                    if c == '\\' {
                        i += 1;
                    } else if c == '\'' {
                        *state = LexerState::Normal;
                    }
                }
                LexerState::InBlockComment(depth) => match (c, next_c) {
                    ('/', Some('*')) => {
                        *state = LexerState::InBlockComment(*depth + 1);
                        i += 1;
                    }
                    ('*', Some('/')) => {
                        if *depth == 1 {
                            *state = LexerState::Normal;
                        } else {
                            *state = LexerState::InBlockComment(*depth - 1);
                        }
                        i += 1;
                    }
                    _ => {}
                },
                LexerState::InLineComment => unreachable!(), // Déjà géré par le break
            }
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconciler_extraction_logic() {
        let content = "
// @raise-handle: fn:test
pub fn test() {
    println!(\"hello\");
}

// @raise-handle: fn:other
fn other() {}
";
        let lines: Vec<&str> = content.lines().collect();
        let (el, next) = Reconciler::extract_element_at("fn:test", &lines, 2).unwrap();
        assert_eq!(el.handle, "fn:test");
        assert_eq!(el.visibility, Visibility::Public);
        assert!(el.body.unwrap().contains("println!"));

        let (el2, _) = Reconciler::extract_element_at("fn:other", &lines, next + 1).unwrap();
        assert_eq!(el2.handle, "fn:other");
    }

    #[test]
    fn test_reconciler_extreme_braces_in_strings_and_comments() {
        let content = r#"
// @raise-handle: fn:hardcore
pub fn hardcore() {
    let a = "{"; // Accolade dans un string
    let b = '}'; // Accolade dans un char
    // Penser à fermer la {
    /* Ou un bloc de commentaire avec plein de { { { 
    */
    if true {
        println!("} est fermé");
    }
}
"#;
        let lines: Vec<&str> = content.lines().collect();
        let (el, _) = Reconciler::extract_element_at("fn:hardcore", &lines, 2).unwrap();

        // Le parseur a survécu et a correctement extrait le corps !
        assert!(el.body.unwrap().contains("println"));
    }
}
