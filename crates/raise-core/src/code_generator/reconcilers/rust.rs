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
    RawStringLit(&'a str),
    CharLit(&'a str),
    Lifetime(&'a str), // Support des lifetimes ('a, 'static)
    LineComment(&'a str),
    BlockComment(&'a str),
    Whitespace(&'a str),
}

impl<'a> Token<'a> {
    fn as_str(&self) -> &'a str {
        match self {
            Token::Ident(s)
            | Token::StringLit(s)
            | Token::RawStringLit(s)
            | Token::CharLit(s)
            | Token::Lifetime(s) // 🎯 NOUVEAU
            | Token::LineComment(s)
            | Token::BlockComment(s)
            | Token::Whitespace(s) => s,
            Token::Symbol(_) => "",
        }
    }
}

struct Lexer<'a> {
    source: &'a str,
    chars: DataStreamPeekable<TextCharIndices<'a>>,
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
                '\'' => {
                    // 🎯 FIX : Différencier un CharLit ('a') d'une Lifetime ('a)
                    let mut lookahead = self.chars.clone();
                    lookahead.next(); // Passe le '\''
                    let mut is_lifetime = false;

                    if let Some(&(_, c1)) = lookahead.peek() {
                        if c1.is_alphabetic() || c1 == '_' {
                            lookahead.next();
                            if let Some(&(_, c2)) = lookahead.peek() {
                                if c2 != '\'' {
                                    is_lifetime = true;
                                }
                            } else {
                                is_lifetime = true;
                            }
                        }
                    }

                    if is_lifetime {
                        tokens.push(self.read_lifetime(start_idx));
                    } else {
                        tokens.push(self.read_char_lit(start_idx));
                    }
                }
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

    fn read_lifetime(&mut self, start: usize) -> Token<'a> {
        self.chars.next(); // '\''
        while let Some(&(_, c)) = self.chars.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.chars.next();
            } else {
                break;
            }
        }
        Token::Lifetime(&self.source[start..self.current_index()])
    }

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
                    let full_tag = comment.replace("// @raise-handle:", "").trim().to_string();

                    // 🎯 L'ANCRE IMMUABLE : L'ID devient le seul vrai "handle" pour la DB
                    let handle = if let Some(start) = full_tag.find("[id: ") {
                        if let Some(end) = full_tag[start..].find(']') {
                            full_tag[start + 5..start + end].to_string()
                        } else {
                            full_tag
                        }
                    } else {
                        full_tag
                    };

                    i += 1; // Pointe sur le début de l'élément (docs, attributs ou signature)

                    // 🎯 FIX : On extrait l'élément, mais on ignore l'index de fin (_next_index)
                    let (element, _next_index) = Self::extract_element(&handle, &tokens, i)?;
                    elements.push(element);

                    // 🚀 On laisse la boucle continuer naturellement à `i`.
                    // Le parseur va donc traverser l'intérieur des `impl` et découvrir les sous-tags !
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

    /// 🚀 AUTO-TAGGING & GARBAGE COLLECTOR : Injecte, corrige et nettoie les ancres sémantiques.
    pub async fn auto_tag_file(path: &Path) -> RaiseResult<usize> {
        let content = match fs::read_to_string_async(path).await {
            Ok(c) => c,
            Err(e) => raise_error!(
                "ERR_SYSTEM_IO",
                error = e,
                context = json_value!({ "action": "read_file_for_tagging", "path": path.display().to_string() })
            ),
        };

        let mut lexer = Lexer::new(&content);
        let tokens = lexer.tokenize();

        // 🎯 L'Éditeur Chirurgical : (offset_binaire, longueur_à_remplacer, nouveau_texte)
        let mut edits: Vec<(usize, usize, String)> = Vec::new();

        // 🧹 Suivi pour le Garbage Collector
        let mut all_existing_tags: UniqueSet<usize> = UniqueSet::new();
        let mut used_tags: UniqueSet<usize> = UniqueSet::new();

        for token in &tokens {
            if let Token::LineComment(c) = token {
                if c.starts_with("// @raise-handle:") {
                    let offset = (c.as_ptr() as usize) - (content.as_ptr() as usize);
                    all_existing_tags.insert(offset);
                }
            }
        }

        let mut brace_depth: usize = 0;
        let mut element_start_idx = 0;
        let mut i = 0;

        let mut test_scope_depth: Option<usize> = None;

        while i < tokens.len() {
            let token = &tokens[i];

            match token {
                Token::Symbol('{') => {
                    brace_depth += 1;
                    if brace_depth <= 1 && test_scope_depth.is_none() {
                        element_start_idx = i + 1;
                    }
                }
                Token::Symbol('}') => {
                    // 🚪 Sortie de la zone de quarantaine
                    if let Some(depth) = test_scope_depth {
                        if brace_depth == depth + 1 {
                            test_scope_depth = None;
                        }
                    }

                    brace_depth = brace_depth.saturating_sub(1);
                    if brace_depth <= 1 && test_scope_depth.is_none() {
                        element_start_idx = i + 1;
                    }
                }
                Token::Symbol(';') if brace_depth <= 1 && test_scope_depth.is_none() => {
                    element_start_idx = i + 1;
                }
                Token::Ident(kw) => {
                    // 🛡️ DÉTECTION DU MODULE DE TESTS
                    if *kw == "mod" {
                        let mut k = i + 1;
                        while k < tokens.len() {
                            match &tokens[k] {
                                Token::Ident(n) if *n == "tests" => {
                                    test_scope_depth = Some(brace_depth);
                                    break;
                                }
                                Token::Whitespace(_) => k += 1,
                                _ => break,
                            }
                        }
                    }

                    let is_target = match *kw {
                        // Les structures de haut niveau ne sont taguées que hors des tests
                        "struct" | "enum" | "impl" | "trait" | "type" | "macro_rules" => {
                            brace_depth == 0 && test_scope_depth.is_none()
                        }
                        // Les fonctions sont taguées si elles sont de niveau 1, OU de niveau 2 dans un mod tests
                        "fn" => {
                            brace_depth <= 1
                                || (test_scope_depth.is_some()
                                    && brace_depth == test_scope_depth.unwrap() + 1)
                        }
                        _ => false,
                    };

                    if is_target {
                        // --- 1. EXTRACTION DU NOM (Avec support 'impl ... for ...') ---
                        let mut name = String::new();
                        let mut k = i + 1;

                        if *kw == "impl" {
                            let mut trait_name = String::new();
                            let mut target_name = String::new();
                            let mut has_for = false;

                            while k < tokens.len() {
                                match &tokens[k] {
                                    Token::Symbol('{') => break,
                                    Token::Ident(n) => {
                                        if *n == "for" {
                                            has_for = true;
                                        } else if has_for && target_name.is_empty() {
                                            target_name = n.to_string();
                                        } else if !has_for && trait_name.is_empty() {
                                            trait_name = n.to_string();
                                        }
                                    }
                                    _ => {}
                                }
                                k += 1;
                            }

                            if has_for && !target_name.is_empty() {
                                name = format!("{}_{}", target_name, trait_name);
                            } else {
                                name = trait_name;
                            }
                        } else {
                            while k < tokens.len() {
                                if let Token::Ident(n) = &tokens[k] {
                                    name = n.to_string();
                                    break;
                                }
                                k += 1;
                            }
                        }

                        // --- 2. VÉRIFICATION & RÉCONCILIATION DU TAG ---
                        if !name.is_empty() {
                            let mut tag_type = match *kw {
                                "fn" => "fn",
                                "struct" => "struct",
                                "enum" => "enum",
                                "impl" => "impl",
                                "trait" => "trait",
                                "type" => "type",
                                "macro_rules" => "macro",
                                _ => "unknown",
                            };

                            if test_scope_depth.is_some() && *kw == "fn" {
                                tag_type = "test";
                            }

                            let mut found_existing_tag = None;
                            for token in &tokens[element_start_idx..i] {
                                if let Token::LineComment(c) = token {
                                    if c.starts_with("// @raise-handle:") {
                                        found_existing_tag = Some(*c);
                                        break;
                                    }
                                }
                            }

                            // 🧬 GÉNÉRATION OU RÉCUPÉRATION DE L'ADN IMMUABLE (UUID)
                            let id_marker = if let Some(existing_c) = found_existing_tag {
                                if let Some(start) = existing_c.find("[id: ") {
                                    if let Some(end) = existing_c[start..].find(']') {
                                        // Le tag a déjà un ID, on le préserve coûte que coûte !
                                        existing_c[start..start + end + 1].to_string()
                                    } else {
                                        format!("[id: {}]", &UniqueId::new_v4().to_string()[0..8])
                                    }
                                } else {
                                    // Ancien tag sans ID, on le met à niveau
                                    format!("[id: {}]", &UniqueId::new_v4().to_string()[0..8])
                                }
                            } else {
                                // Nouveau tag, on génère un ID frais
                                format!("[id: {}]", &UniqueId::new_v4().to_string()[0..8])
                            };

                            // Le nom sert à l'humain, l'ID sert à la machine
                            let expected_tag_content =
                                format!("// @raise-handle: {}:{} {}", tag_type, name, id_marker);

                            if let Some(existing_c) = found_existing_tag {
                                let offset =
                                    (existing_c.as_ptr() as usize) - (content.as_ptr() as usize);
                                used_tags.insert(offset);

                                // 🛠️ AUTO-CORRECTION: Le tag existe mais est désynchronisé (ex: renommage de la fonction)
                                if existing_c != expected_tag_content {
                                    edits.push((offset, existing_c.len(), expected_tag_content));
                                }
                            } else {
                                // ➕ INJECTION: Aucun tag n'existe, on le crée
                                let mut insert_token_idx = i;
                                for (offset, token) in
                                    tokens[element_start_idx..i].iter().enumerate()
                                {
                                    match token {
                                        Token::Whitespace(_) => continue,
                                        Token::LineComment(c) if !c.starts_with("///") => continue,
                                        Token::BlockComment(_) => continue,
                                        _ => {
                                            insert_token_idx = element_start_idx + offset;
                                            break;
                                        }
                                    }
                                }

                                let mut offset = 0;
                                for k in insert_token_idx..tokens.len() {
                                    let s = tokens[k].as_str();
                                    if !s.is_empty() {
                                        offset =
                                            (s.as_ptr() as usize) - (content.as_ptr() as usize);
                                        for token in &tokens[insert_token_idx..k] {
                                            if let Token::Symbol(c) = token {
                                                offset -= c.len_utf8();
                                            }
                                        }
                                        break;
                                    }
                                }

                                let expected_tag_with_newline =
                                    format!("{}\n", expected_tag_content);
                                edits.push((offset, 0, expected_tag_with_newline));
                            }
                        }
                        element_start_idx = i + 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }

        // --- 3. GARBAGE COLLECTOR : Nettoyage des fantômes ---
        for offset in all_existing_tags.iter() {
            if !used_tags.contains(offset) {
                // Trouver la taille du tag orphelin pour l'effacer
                for token in &tokens {
                    if let Token::LineComment(c) = token {
                        let t_offset = (c.as_ptr() as usize) - (content.as_ptr() as usize);
                        if t_offset == *offset {
                            let mut len_to_remove = c.len();
                            // 🧹 On efface aussi le saut de ligne qui suit pour ne pas laisser de ligne vide
                            if let Some(&b'\n') = content.as_bytes().get(t_offset + len_to_remove) {
                                len_to_remove += 1;
                            }
                            edits.push((*offset, len_to_remove, String::new()));
                            break;
                        }
                    }
                }
            }
        }

        // --- 4. APPLICATION DES ÉDITS IN-PLACE (Zéro Dette) ---
        let edits_count = edits.len();
        if edits_count > 0 {
            // On trie du bas vers le haut pour ne jamais fausser les offsets lors des remplacements
            edits.sort_by_key(|k| ReverseOrder(k.0));
            let mut modified_content = content.clone();

            for (offset, len_to_remove, new_text) in edits {
                if len_to_remove > 0 {
                    modified_content.replace_range(offset..(offset + len_to_remove), &new_text);
                } else {
                    modified_content.insert_str(offset, &new_text);
                }
            }

            if let Err(e) = fs::write_async(path, &modified_content).await {
                raise_error!("ERR_AUTO_TAG_WRITE_FAILED", error = e);
            }
            let _ = crate::utils::io::os::exec_command_async(
                "rustfmt",
                &["--edition", "2021", path.to_string_lossy().as_ref()],
                None,
            )
            .await;
        }

        Ok(edits_count)
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
