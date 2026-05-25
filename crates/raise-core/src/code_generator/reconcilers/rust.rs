use crate::code_generator::models::{CodeElement, CodeElementType, Visibility};
use crate::utils::prelude::*;

// =========================================================================
// 1. LE LEXER (TOKENIZER)
// Rôle : Découper le texte brut en morceaux typés sans chercher à les comprendre.
// Il neutralise le piège des accolades dans les strings/commentaires.
// =========================================================================

#[derive(Debug, PartialEq, Clone)]
enum Token {
    Ident(String),
    Symbol(char),
    StringLit(String),
    CharLit(String),
    LineComment(String),
    BlockComment(String),
    Whitespace(String),
}

impl Token {
    /// Permet de reconstituer le code source exact.
    fn as_str(&self) -> String {
        match self {
            Token::Ident(s)
            | Token::StringLit(s)
            | Token::CharLit(s)
            | Token::LineComment(s)
            | Token::BlockComment(s)
            | Token::Whitespace(s) => s.clone(),
            Token::Symbol(c) => c.to_string(),
        }
    }
}

struct Lexer<'a> {
    chars: DataStreamPeekable<TextChars<'a>>,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
        }
    }

    fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        while let Some(&c) = self.chars.peek() {
            match c {
                c if c.is_whitespace() => tokens.push(self.read_whitespace()),
                c if c.is_alphabetic() || c == '_' => tokens.push(self.read_ident()),
                '"' => tokens.push(self.read_string_lit()),
                '\'' => tokens.push(self.read_char_lit()),
                '/' => {
                    self.chars.next(); // Consomme le 1er '/'
                    match self.chars.peek() {
                        Some(&'/') => tokens.push(self.read_line_comment()),
                        Some(&'*') => tokens.push(self.read_block_comment()),
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

    fn read_whitespace(&mut self) -> Token {
        let mut s = String::new();
        while let Some(&c) = self.chars.peek() {
            if c.is_whitespace() {
                s.push(c);
                self.chars.next();
            } else {
                break;
            }
        }
        Token::Whitespace(s)
    }

    fn read_ident(&mut self) -> Token {
        let mut s = String::new();
        while let Some(&c) = self.chars.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.chars.next();
            } else {
                break;
            }
        }
        Token::Ident(s)
    }

    fn read_string_lit(&mut self) -> Token {
        let mut s = String::new();
        s.push(self.chars.next().unwrap()); // '"'
        while let Some(&c) = self.chars.peek() {
            s.push(self.chars.next().unwrap());
            if c == '\\' {
                if let Some(next_c) = self.chars.next() {
                    s.push(next_c);
                }
            } else if c == '"' {
                break;
            }
        }
        Token::StringLit(s)
    }

    fn read_char_lit(&mut self) -> Token {
        let mut s = String::new();
        s.push(self.chars.next().unwrap()); // '\''
        while let Some(&c) = self.chars.peek() {
            s.push(self.chars.next().unwrap());
            if c == '\\' {
                if let Some(next_c) = self.chars.next() {
                    s.push(next_c);
                }
            } else if c == '\'' {
                break;
            }
        }
        Token::CharLit(s)
    }

    fn read_line_comment(&mut self) -> Token {
        let mut s = String::from("/");
        while let Some(&c) = self.chars.peek() {
            if c == '\n' {
                break;
            }
            s.push(self.chars.next().unwrap());
        }
        Token::LineComment(s)
    }

    fn read_block_comment(&mut self) -> Token {
        let mut s = String::from("/");
        s.push(self.chars.next().unwrap()); // '*'
        while self.chars.peek().is_some() {
            s.push(self.chars.next().unwrap());
            if s.ends_with("*/") {
                break;
            }
        }
        Token::BlockComment(s)
    }
}

// =========================================================================
// 2. LE PARSER (AST SHALLOW EXTRACTOR)
// Rôle : Parcourir les Tokens pour assembler le "Jumeau Numérique".
// =========================================================================

pub struct Reconciler;

impl Reconciler {
    /// 📂 Lit un fichier physique et délègue au parseur sémantique.
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

    /// 🧠 Extrait les éléments de code depuis une chaîne de caractères via Tokenisation.
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
                        attr_str.push_str(&t.as_str());

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

        // 2. Extraction de la Signature
        let mut signature_tokens = Vec::new();
        let mut body_start_index = None;
        let mut has_body = false;

        while i < tokens.len() {
            let t = &tokens[i];
            if t == &Token::Symbol('{') {
                body_start_index = Some(i);
                has_body = true;
                break;
            } else if t == &Token::Symbol(';') {
                signature_tokens.push(t.clone());
                i += 1;
                break;
            } else {
                signature_tokens.push(t.clone());
                i += 1;
            }
        }

        let signature = signature_tokens
            .into_iter()
            .map(|t| t.as_str())
            .collect::<String>()
            .trim()
            .to_string();

        // Analyse sémantique de la signature
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
        } else if signature.contains("type ") {
            CodeElementType::TypeAlias
        } else if signature.contains("macro_rules! ") {
            CodeElementType::Macro
        } else if signature.contains("const ") {
            CodeElementType::Constant
        } else {
            CodeElementType::Function
        }; // Fallback par défaut

        // 3. Extraction du Corps (Body) via comptage strict d'accolades
        let mut body = None;
        if has_body {
            if let Some(mut start) = body_start_index {
                let mut brace_count = 0;
                let mut body_str = String::new();

                while start < tokens.len() {
                    let t = &tokens[start];
                    body_str.push_str(&t.as_str());

                    // Seuls les symboles purs sont comptés
                    if t == &Token::Symbol('{') {
                        brace_count += 1;
                    }
                    if t == &Token::Symbol('}') {
                        brace_count -= 1;
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

        // 4. Création de l'élément complet avec l'ontologie RAISE
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
// TESTS UNITAIRES
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

    #[test]
    fn test_reconciler_no_body_struct() {
        let code = r#"
// @raise-handle: struct:unit
pub(crate) struct Unit;
"#;
        let elements = Reconciler::parse_content(code).unwrap();
        assert_eq!(elements.len(), 1);

        let el = &elements[0];
        assert_eq!(el.visibility, Visibility::Crate);
        assert_eq!(el.element_type, CodeElementType::Struct);
        assert_eq!(el.signature, "pub(crate) struct Unit;");
        assert_eq!(el.body, None);
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
            assert_eq!(
                data.context.get("handle").unwrap().as_str().unwrap(),
                "fn:broken"
            );
        } else {
            panic!("Le type d'erreur n'est pas AppError::Structured");
        }
    }
}
