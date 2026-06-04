use crate::code_generator::models::{CodeElement, CodeElementType, Visibility};
use crate::utils::prelude::*;

// =========================================================================
// 1. LE LEXER ZERO-COPY (TOKENIZER)
// Rôle : Découper le texte brut en morceaux typés sans allocation inutile.
// =========================================================================

#[derive(Debug, PartialEq, Clone)]
enum Token<'a> {
    Ident(&'a str),
    Symbol(char),
    StringLit(&'a str),
    RawStringLit(&'a str), // 🎯 NOUVEAU : Gère les r#"..."#
    CharLit(&'a str),
    LineComment(&'a str),
    BlockComment(&'a str),
    Whitespace(&'a str),
}

impl<'a> Token<'a> {
    /// Permet de reconstituer le code source exact (Zero-Copy extraction).
    fn as_str(&self) -> &'a str {
        match self {
            Token::Ident(s)
            | Token::StringLit(s)
            | Token::RawStringLit(s)
            | Token::CharLit(s)
            | Token::LineComment(s)
            | Token::BlockComment(s)
            | Token::Whitespace(s) => s,
            // Pour le symbole unique, on gère la conversion côté Reconciler pour éviter l'allocation ici
            Token::Symbol(_) => "",
        }
    }
}

struct Lexer<'a> {
    source: &'a str,
    // 🎯 FIX : On utilise char_indices pour tracker la position exacte en octets
    chars: DataStreamPeekable<std::str::CharIndices<'a>>,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            source: input,
            chars: input.char_indices().peekable(),
        }
    }

    /// Récupère l'index de l'octet courant
    fn current_index(&mut self) -> usize {
        self.chars
            .peek()
            .map(|&(i, _)| i)
            .unwrap_or(self.source.len())
    }

    fn tokenize(&mut self) -> Vec<Token<'a>> {
        let mut tokens = Vec::new();

        while let Some(&(start_idx, c)) = self.chars.peek() {
            match c {
                c if c.is_whitespace() => tokens.push(self.read_whitespace(start_idx)),
                c if c.is_alphabetic() || c == '_' => {
                    // 🎯 FIX : Détection des Raw Strings (ex: r#"..."#)
                    if c == 'r' {
                        let mut lookahead = self.chars.clone();
                        lookahead.next(); // Passe le 'r'
                        if let Some(&(_, next_c)) = lookahead.peek() {
                            if next_c == '#' || next_c == '"' {
                                if let Some(tok) = self.read_raw_string(start_idx) {
                                    tokens.push(tok);
                                    continue;
                                }
                            }
                        }
                    }
                    tokens.push(self.read_ident(start_idx))
                }
                '"' => tokens.push(self.read_string_lit(start_idx)),
                '\'' => tokens.push(self.read_char_lit(start_idx)),
                '/' => {
                    self.chars.next(); // Consomme le 1er '/'
                    match self.chars.peek() {
                        Some(&(_, '/')) => tokens.push(self.read_line_comment(start_idx)),
                        Some(&(_, '*')) => tokens.push(self.read_block_comment(start_idx)),
                        _ => tokens.push(Token::Symbol('/')),
                    }
                }
                _ => {
                    tokens.push(Token::Symbol(c));
                    self.chars.next();
                }
            }
        }
        tokens
    }

    fn read_whitespace(&mut self, start: usize) -> Token<'a> {
        while let Some(&(_, c)) = self.chars.peek() {
            if c.is_whitespace() {
                self.chars.next();
            } else {
                break;
            }
        }
        Token::Whitespace(&self.source[start..self.current_index()])
    }

    fn read_ident(&mut self, start: usize) -> Token<'a> {
        while let Some(&(_, c)) = self.chars.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.chars.next();
            } else {
                break;
            }
        }
        Token::Ident(&self.source[start..self.current_index()])
    }

    fn read_string_lit(&mut self, start: usize) -> Token<'a> {
        self.chars.next(); // '"'
        while let Some(&(_, c)) = self.chars.peek() {
            self.chars.next();
            if c == '\\' {
                self.chars.next();
            }
            // Skip escaped char
            else if c == '"' {
                break;
            }
        }
        Token::StringLit(&self.source[start..self.current_index()])
    }

    // 🎯 FIX : Implémentation de la lecture des Raw Strings
    fn read_raw_string(&mut self, start: usize) -> Option<Token<'a>> {
        self.chars.next(); // 'r'
        let mut hashes = 0;

        while let Some(&(_, c)) = self.chars.peek() {
            if c == '#' {
                hashes += 1;
                self.chars.next();
            } else if c == '"' {
                self.chars.next();
                break;
            } else {
                return None;
            } // Invalide
        }

        while let Some(&(_, c)) = self.chars.peek() {
            self.chars.next();
            if c == '"' {
                let mut closing_hashes = 0;
                let mut lookahead = self.chars.clone();
                for _ in 0..hashes {
                    if let Some(&(_, '#')) = lookahead.peek() {
                        closing_hashes += 1;
                        lookahead.next();
                    }
                }
                if closing_hashes == hashes {
                    for _ in 0..hashes {
                        self.chars.next();
                    } // Consomme les # de fin
                    return Some(Token::RawStringLit(
                        &self.source[start..self.current_index()],
                    ));
                }
            }
        }
        None
    }

    fn read_char_lit(&mut self, start: usize) -> Token<'a> {
        self.chars.next(); // '\''
        while let Some(&(_, c)) = self.chars.peek() {
            self.chars.next();
            if c == '\\' {
                self.chars.next();
            } else if c == '\'' {
                break;
            }
        }
        Token::CharLit(&self.source[start..self.current_index()])
    }

    fn read_line_comment(&mut self, start: usize) -> Token<'a> {
        while let Some(&(_, c)) = self.chars.peek() {
            if c == '\n' {
                break;
            }
            self.chars.next();
        }
        Token::LineComment(&self.source[start..self.current_index()])
    }

    fn read_block_comment(&mut self, start: usize) -> Token<'a> {
        self.chars.next(); // '*'
        let mut prev = '\0';
        while let Some(&(_, c)) = self.chars.peek() {
            self.chars.next();
            if prev == '*' && c == '/' {
                break;
            }
            prev = c;
        }
        Token::BlockComment(&self.source[start..self.current_index()])
    }
}

// =========================================================================
// 2. LE PARSER (AST SHALLOW EXTRACTOR)
// =========================================================================

pub struct Reconciler;

impl Reconciler {
    pub async fn parse_from_file(path: &Path) -> RaiseResult<Vec<CodeElement>> {
        let content = match fs::read_to_string_async(path).await {
            Ok(c) => c,
            Err(e) => raise_error!(
                "ERR_SYSTEM_IO",
                error = e,
                context = json_value!({ "action": "read_file_async", "path": path.display().to_string() })
            ),
        };
        Self::parse_content(&content)
    }

    pub fn parse_content(content: &str) -> RaiseResult<Vec<CodeElement>> {
        let mut lexer = Lexer::new(content);
        let tokens = lexer.tokenize();
        let mut elements = Vec::new();
        let mut i = 0;

        while i < tokens.len() {
            if let Token::LineComment(comment) = &tokens[i] {
                if comment.starts_with("// @raise-handle:") {
                    let handle = comment.replace("// @raise-handle:", "").trim().to_string();
                    i += 1;

                    let (element, next_index) = Self::extract_element(&handle, &tokens, i)?;
                    elements.push(element);
                    i = next_index;
                    continue;
                }
            }
            i += 1;
        }

        Ok(elements)
    }

    fn extract_element(
        handle: &str,
        tokens: &[Token],
        start_index: usize,
    ) -> RaiseResult<(CodeElement, usize)> {
        let mut i = start_index;
        let mut docs = String::new();
        let mut attributes = Vec::new();

        // 1. Extraction des Métadonnées (Docs et Attributs)
        while i < tokens.len() {
            match &tokens[i] {
                Token::Whitespace(_) => i += 1,
                Token::LineComment(c) if c.starts_with("///") => {
                    docs.push_str(c.trim_start_matches("///").trim());
                    docs.push('\n');
                    i += 1;
                }
                Token::Symbol('#') => {
                    let mut attr_str = String::new();
                    let mut bracket_count = 0;
                    let mut in_attr = false;

                    while i < tokens.len() {
                        let t = &tokens[i];
                        if let Token::Symbol(sym) = t {
                            attr_str.push(*sym);
                        } else {
                            attr_str.push_str(t.as_str());
                        }

                        if t == &Token::Symbol('[') {
                            bracket_count += 1;
                            in_attr = true;
                        } else if t == &Token::Symbol(']') {
                            bracket_count -= 1;
                        }

                        i += 1;
                        if in_attr && bracket_count == 0 {
                            break;
                        }
                    }
                    attributes.push(attr_str.trim().to_string());
                }
                _ => break, // On a atteint la signature
            }
        }

        // 2. Extraction de la Signature (Ignorant les commentaires normaux)
        let mut signature_str = String::new();
        let mut body_start_index = None;
        let mut has_body = false;

        while i < tokens.len() {
            let t = &tokens[i];

            // 🎯 FIX : On ignore les commentaires intra-signature (ex: /* arg */)
            if let Token::BlockComment(_) = t {
                i += 1;
                continue;
            }
            if let Token::LineComment(c) = t {
                if !c.starts_with("///") {
                    i += 1;
                    continue;
                }
            }

            if t == &Token::Symbol('{') {
                body_start_index = Some(i);
                has_body = true;
                break;
            } else if t == &Token::Symbol(';') {
                signature_str.push(';');
                i += 1;
                break;
            } else {
                if let Token::Symbol(sym) = t {
                    signature_str.push(*sym);
                } else {
                    signature_str.push_str(t.as_str());
                }
                i += 1;
            }
        }

        let signature = signature_str.trim().to_string();

        let visibility = if signature.starts_with("pub(crate)") {
            Visibility::Crate
        } else if signature.starts_with("pub ") {
            Visibility::Public
        } else {
            Visibility::Private
        };

        let element_type = if signature.contains("fn ") {
            CodeElementType::Function
        } else if signature.contains("struct ") {
            CodeElementType::Struct
        } else if signature.contains("trait ") {
            CodeElementType::Trait
        } else if signature.contains("impl ") {
            CodeElementType::ImplBlock
        } else if signature.contains("enum ") {
            CodeElementType::Enum
        } else if signature.contains("macro_rules! ") {
            CodeElementType::Macro
        } else {
            CodeElementType::Function
        };

        // 3. Extraction du Corps (Body)
        let mut body = None;
        if has_body {
            if let Some(mut start) = body_start_index {
                let mut brace_count = 0;
                let mut body_str = String::new();

                while start < tokens.len() {
                    let t = &tokens[start];

                    if let Token::Symbol(sym) = t {
                        body_str.push(*sym);
                        if *sym == '{' {
                            brace_count += 1;
                        }
                        if *sym == '}' {
                            brace_count -= 1;
                        }
                    } else {
                        body_str.push_str(t.as_str());
                    }

                    start += 1;
                    if brace_count == 0 {
                        break;
                    }
                }

                if brace_count != 0 {
                    raise_error!(
                        "ERR_RECONCILER_UNBALANCED_BRACES",
                        error = "Accolades non équilibrées détectées dans le corps de l'élément.",
                        context = json_value!({ "handle": handle })
                    );
                }
                body = Some(body_str.trim().to_string());
                i = start;
            }
        }

        let element = CodeElement {
            module_id: None,
            parent_id: None,
            element_type,
            handle: handle.to_string(),
            visibility,
            attributes,
            docs: if docs.is_empty() {
                None
            } else {
                Some(docs.trim().to_string())
            },
            signature,
            body,
            elements: Vec::new(),
            dependencies: Vec::new(),
            metadata: UnorderedMap::new(),
        };

        Ok((element, i))
    }
}

// =========================================================================
// TESTS UNITAIRES (Fiabilisés)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconciler_ast_perfect_extraction() {
        let code = r#"
// @raise-handle: fn:complex_logic
/// Doc ligne 1
/// Doc ligne 2
#[async_test]
#[cfg(feature = "ai")]
pub async fn complex_logic() -> Result<(), Error> {
    let a = 1;
}
"#;
        let elements = Reconciler::parse_content(code).unwrap();
        assert_eq!(elements.len(), 1);
        let el = &elements[0];

        assert_eq!(el.handle, "fn:complex_logic");
        assert_eq!(el.visibility, Visibility::Public);
        assert_eq!(el.element_type, CodeElementType::Function);
        assert_eq!(el.docs.as_deref().unwrap(), "Doc ligne 1\nDoc ligne 2");
        assert_eq!(
            el.attributes,
            vec!["#[async_test]", "#[cfg(feature = \"ai\")]"]
        );
        assert_eq!(
            el.signature,
            "pub async fn complex_logic() -> Result<(), Error>"
        );
        assert_eq!(el.body.as_deref().unwrap(), "{\n    let a = 1;\n}");
    }

    #[test]
    fn test_reconciler_lexer_destroys_string_brace_bug() {
        let code = r#"
// @raise-handle: fn:trap
fn trap() {
    let s = "{ une accolade piège }"; // Un commentaire avec {
    /* Un bloc avec } */
    let c = '{';
}
"#;
        let elements = Reconciler::parse_content(code).unwrap();
        assert_eq!(elements.len(), 1);

        let el = &elements[0];
        assert!(el
            .body
            .as_deref()
            .unwrap()
            .contains("{ une accolade piège }"));
        assert!(el.body.as_deref().unwrap().contains("let c = '{';"));
    }

    // 🎯 NOUVEAU TEST 1 : Robustesse face aux Raw Strings
    // FIX de formatage : on utilise r##"..."## pour englober la string, car
    // le code Rust simulé contient lui-même un r#"..."#.
    #[test]
    fn test_reconciler_zero_copy_raw_strings() {
        let code = r##"
// @raise-handle: fn:raw_string_test
fn raw_string_test() {
    // Si le parseur ne gère pas les raw strings, cette accolade désynchronise le compteur
    let regex = r#"(?x) { \d+ }"#; 
}
"##;
        let elements = Reconciler::parse_content(code).unwrap();
        assert_eq!(
            elements.len(),
            1,
            "Le parsing ne doit pas échouer sur un Stack Overflow ou une désynchronisation"
        );
        assert!(elements[0]
            .body
            .as_deref()
            .unwrap()
            .contains(r##"r#"(?x) { \d+ }"#"##));
    }

    // 🎯 NOUVEAU TEST 2 : Robustesse intra-signature
    #[test]
    fn test_reconciler_comments_in_signature() {
        let code = r#"
// @raise-handle: fn:comment_in_sig
pub fn with_comment(
    /* identifiant de session */
    session_id: u32
) {
    println!("ok");
}
"#;
        let elements = Reconciler::parse_content(code).unwrap();
        assert_eq!(elements.len(), 1);

        let el = &elements[0];
        // Le parseur doit avoir ignoré le "/* identifiant de session */" lors de l'assemblage de la signature
        assert!(!el.signature.contains("identifiant de session"));
        assert_eq!(
            el.signature,
            "pub fn with_comment(\n    \n    session_id: u32\n)" // La structure est préservée
        );
    }

    #[test]
    fn test_reconciler_unbalanced_error_handled_by_raise() {
        let code = r#"
// @raise-handle: fn:broken
fn broken() {
    let a = 1;
// Missing closing brace
"#;
        let result = Reconciler::parse_content(code);
        assert!(result.is_err(), "Devrait retourner une erreur RAISE");

        if let Err(AppError::Structured(data)) = result {
            assert_eq!(data.code, "ERR_RECONCILER_UNBALANCED_BRACES");
        } else {
            panic!("Le type d'erreur n'est pas AppError::Structured");
        }
    }
}
