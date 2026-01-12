use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Analyseur capable d'extraire des blocs de code protégés d'un fichier existant.
pub struct InjectionAnalyzer;

impl InjectionAnalyzer {
    /// Lit un fichier et extrait le contenu entre les marqueurs.
    /// Les templates doivent utiliser la syntaxe:
    /// `// AI_INJECTION_POINT: [Clé]` ... `// END_AI_INJECTION_POINT`
    /// ou `-- AI_INJECTION_POINT: [Clé]` (pour SQL/VHDL/Lua)
    pub fn extract_injections(file_path: &Path) -> Result<HashMap<String, String>> {
        let mut injections = HashMap::new();

        if !file_path.exists() {
            return Ok(injections);
        }

        let content = fs::read_to_string(file_path)?;

        // Regex robuste :
        // 1. (?:^|\n)\s* -> Début de ligne avec espaces optionnels
        // 2. (?://|--|#) -> Commentaire (// ou -- ou #)
        // 3. \s*AI_INJECTION_POINT:\s*(\w+) -> Le marqueur et la clé (Groupe 1)
        // 4. (.*?) -> Le contenu à capturer (Groupe 2) en mode "dot matches newline" (?s)
        // 5. (?://|--|#)\s*END_... -> Le marqueur de fin (non capturé) ou fin de fichier

        // Note: L'implémentation Regex Rust par défaut ne supporte pas le lookaround,
        // on fait donc une approche itérative simple.

        let start_pattern =
            Regex::new(r"(?m)^\s*(?://|--|#)\s*AI_INJECTION_POINT:\s*(\w+)\s*$").unwrap();
        // On cherchera la fin manuellement pour plus de robustesse sur le contenu multilingue

        let lines: Vec<&str> = content.lines().collect();
        let mut current_key: Option<String> = None;
        let mut current_block: Vec<String> = Vec::new();

        for line in lines {
            if let Some(captures) = start_pattern.captures(line) {
                // On a trouvé un début de bloc
                if let Some(key) = captures.get(1) {
                    current_key = Some(key.as_str().to_string());
                    current_block.clear();
                }
                continue;
            }

            // Vérification de la fin de bloc
            // On accepte "END_AI_INJECTION_POINT" précédé d'un commentaire
            if line.contains("END_AI_INJECTION_POINT")
                && (line.trim().starts_with("//")
                    || line.trim().starts_with("--")
                    || line.trim().starts_with("#"))
            {
                if let Some(key) = current_key.take() {
                    // On sauvegarde le bloc nettoyé (join avec \n)
                    // On trim le premier/dernier saut de ligne vide souvent ajouté par l'éditeur
                    let block_content = current_block.join("\n");
                    injections.insert(key, block_content.trim().to_string());
                }
                continue;
            }

            // Si on est dans un bloc, on capture
            if current_key.is_some() {
                current_block.push(line.to_string());
            }
        }

        Ok(injections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_extract_rust_injection() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
            fn main() {{}}
            
            // AI_INJECTION_POINT: MyLogic
            let x = 10;
            println!("Custom Code");
            // END_AI_INJECTION_POINT
        "#
        )
        .unwrap();

        let injections = InjectionAnalyzer::extract_injections(file.path()).unwrap();

        assert!(injections.contains_key("MyLogic"));
        let code = injections.get("MyLogic").unwrap();
        assert!(code.contains("let x = 10;"));
        assert!(code.contains("println!(\"Custom Code\");"));
    }

    #[test]
    fn test_extract_python_injection() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
            # AI_INJECTION_POINT: PythonHook
            def custom_hook():
                pass
            # END_AI_INJECTION_POINT
        "#
        )
        .unwrap();

        let injections = InjectionAnalyzer::extract_injections(file.path()).unwrap();
        assert!(injections.contains_key("PythonHook"));
        assert!(injections
            .get("PythonHook")
            .unwrap()
            .contains("def custom_hook():"));
    }
}
