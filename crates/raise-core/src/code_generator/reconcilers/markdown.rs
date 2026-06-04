// FICHIER : src-tauri/src/code_generator/reconcilers/markdown.rs

use crate::code_generator::models::{DocElement, DocElementType};
use crate::code_generator::utils::StringUtils;
use crate::utils::prelude::*;

pub struct DocReconciler;

impl DocReconciler {
    /// 📂 Lit un fichier physique et délègue au parseur sémantique.
    pub async fn parse_from_file(path: &Path) -> RaiseResult<Vec<DocElement>> {
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

    /// 🧠 Machine à états O(N) pour transformer le Markdown brut en Jumeau Numérique.
    pub fn parse_content(content: &str) -> RaiseResult<Vec<DocElement>> {
        let mut elements: Vec<DocElement> = Vec::new();
        let mut parent_stack: Vec<(u32, String)> = Vec::new(); // (Niveau, Handle)
        let mut current_section: Option<DocElement> = None;

        let mut in_code_block = false;
        let mut in_frontmatter = false;
        let mut buffer = String::new();
        let mut current_lang = String::new();

        // 🎯 OPTIMISATION O(N) : On stocke les liens parent->enfants à la volée pour éviter les clones
        let mut children_map: UnorderedMap<String, Vec<String>> = UnorderedMap::new();
        // 🎯 SÉCURITÉ : Registre pour éviter les collisions de Handles (ex: 2x "Introduction")
        let mut handle_counts: UnorderedMap<String, usize> = UnorderedMap::new();

        let lines: Vec<&str> = content.lines().collect();

        for (i, &line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // ================================================================
            // 1. GESTION DU FRONTMATTER (Métadonnées YAML au début du fichier)
            // ================================================================
            if i == 0 && trimmed == "---" {
                in_frontmatter = true;
                continue;
            }

            if in_frontmatter {
                if trimmed == "---" {
                    in_frontmatter = false;
                    elements.push(DocElement {
                        module_id: None,
                        parent_id: None,
                        element_type: DocElementType::Frontmatter,
                        handle: "frontmatter".to_string(),
                        title: "Metadata".to_string(),
                        heading_level: None,
                        content: buffer.trim().to_string(),
                        language: "yaml".to_string(),
                        elements: Vec::new(),
                        metadata: UnorderedMap::new(),
                    });
                    buffer.clear();
                } else {
                    buffer.push_str(line);
                    buffer.push('\n');
                }
                continue;
            }

            // ================================================================
            // 2. GESTION DES BLOCS DE CODE (Mermaid / Rust / etc.)
            // ================================================================
            if trimmed.starts_with("```") {
                if !in_code_block {
                    // Avant d'ouvrir un bloc, on clôture la section texte en cours
                    if let Some(mut sec) = current_section.take() {
                        sec.content = sec.content.trim().to_string();
                        elements.push(sec);
                    }
                    in_code_block = true;
                    current_lang = trimmed.trim_start_matches("```").trim().to_string();
                    if current_lang.is_empty() {
                        current_lang = "text".to_string();
                    }
                    buffer.clear();
                } else {
                    // Fermeture et création de l'élément technique
                    in_code_block = false;
                    let el_type = if current_lang == "mermaid" {
                        DocElementType::MermaidDiagram
                    } else {
                        DocElementType::CodeBlock
                    };

                    let parent_id = parent_stack.last().map(|(_, h)| h.clone());
                    let handle = format!("block_{}_{}", current_lang, elements.len());

                    // Enregistrement ultra-rapide du lien de parenté
                    if let Some(ref pid) = parent_id {
                        children_map
                            .entry(pid.clone())
                            .or_default()
                            .push(handle.clone());
                    }

                    elements.push(DocElement {
                        module_id: None,
                        parent_id,
                        element_type: el_type,
                        handle,
                        title: format!("Source {}", current_lang),
                        heading_level: None,
                        content: buffer.clone(),
                        language: current_lang.clone(),
                        elements: Vec::new(),
                        metadata: UnorderedMap::new(),
                    });
                    buffer.clear();
                }
                continue;
            }

            if in_code_block {
                buffer.push_str(line);
                buffer.push('\n');
                continue;
            }

            // ================================================================
            // 3. GESTION DES TITRES ET DE LA HIÉRARCHIE (# Titre)
            // ================================================================
            if trimmed.starts_with('#') {
                let level = trimmed.chars().take_while(|&c| c == '#').count() as u32;
                let title = trimmed.trim_start_matches('#').trim().to_string();

                // 🎯 FIX : Unicité stricte des handles via to_snake_case et compteur
                let base_handle = StringUtils::to_snake_case(&title);
                let handle = if let Some(count) = handle_counts.get_mut(&base_handle) {
                    *count += 1;
                    format!("{}_{}", base_handle, count)
                } else {
                    // 🎯 CORRECTION : On initialise le compteur à 0 pour que le 1er doublon soit _1
                    handle_counts.insert(base_handle.clone(), 0);
                    base_handle
                };

                // Clôture de la section précédente
                if let Some(mut sec) = current_section.take() {
                    sec.content = sec.content.trim().to_string();
                    elements.push(sec);
                }

                // Ajustement de la pile de parenté
                while let Some((p_level, _)) = parent_stack.last() {
                    if *p_level >= level {
                        parent_stack.pop();
                    } else {
                        break;
                    }
                }

                let parent_id = parent_stack.last().map(|(_, h)| h.clone());

                // Enregistrement du lien
                if let Some(ref pid) = parent_id {
                    children_map
                        .entry(pid.clone())
                        .or_default()
                        .push(handle.clone());
                }

                parent_stack.push((level, handle.clone()));

                current_section = Some(DocElement {
                    module_id: None,
                    parent_id,
                    element_type: DocElementType::MarkdownSection,
                    handle,
                    title,
                    heading_level: Some(level),
                    content: String::new(),
                    language: "markdown".to_string(),
                    elements: Vec::new(),
                    metadata: UnorderedMap::new(),
                });
                continue;
            }

            // ================================================================
            // 4. ACCUMULATION DU TEXTE (Avec gestion de l'intro orpheline)
            // ================================================================
            if let Some(ref mut sec) = current_section {
                sec.content.push_str(line);
                sec.content.push('\n');
            } else if !trimmed.is_empty() {
                let handle = "intro".to_string();
                current_section = Some(DocElement {
                    module_id: None,
                    parent_id: None,
                    element_type: DocElementType::MarkdownSection,
                    handle,
                    title: "Introduction".to_string(),
                    heading_level: Some(1),
                    content: line.to_string() + "\n",
                    language: "markdown".to_string(),
                    elements: Vec::new(),
                    metadata: UnorderedMap::new(),
                });
            }
        }

        // Fin de fichier : on clôture la dernière section ouverte
        if let Some(mut sec) = current_section {
            sec.content = sec.content.trim().to_string();
            elements.push(sec);
        }

        // ================================================================
        // 5. SYNCHRONISATION IHM (Zéro Clone, Complexité O(N))
        // ================================================================
        for element in &mut elements {
            if let Some(children) = children_map.remove(&element.handle) {
                element.elements = children;
            }
        }

        Ok(elements)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doc_reconciler_full_hierarchy_optimized() {
        let content = "# Racine\n## A\n### A1\n```mermaid\ngraph TD; A-->B;\n```\n";
        let elements = DocReconciler::parse_content(content).expect("Parsing failed");
        assert!(elements.len() >= 4);

        let section_a = elements.iter().find(|e| e.handle == "a").unwrap();
        // Vérifie que l'enfant 'a1' a bien été rattaché lors de l'assemblage final O(N)
        assert!(section_a.elements.contains(&"a1".to_string()));
    }

    #[test]
    fn test_doc_reconciler_frontmatter_parsing() {
        let content =
            "---\ntitle: Spécification Système\nversion: 1.2\n---\n# Introduction\nTexte.";
        let elements = DocReconciler::parse_content(content).unwrap();

        assert_eq!(elements[0].handle, "frontmatter");
        assert_eq!(elements[0].element_type, DocElementType::Frontmatter);
        assert!(elements[0].content.contains("version: 1.2"));
        assert_eq!(elements[1].handle, "introduction");
    }

    #[test]
    fn test_doc_reconciler_duplicate_headings_resolution() {
        let content = "# Configuration\nTexte 1\n# Configuration\nTexte 2";
        let elements = DocReconciler::parse_content(content).unwrap();

        // Le parseur doit générer un handle unique pour la seconde occurrence
        assert_eq!(elements[0].handle, "configuration");
        assert_eq!(elements[1].handle, "configuration_1");
    }

    #[test]
    fn test_doc_reconciler_orphan_intro() {
        let content = "Introduction sans titre.\n# Titre 1";
        let elements = DocReconciler::parse_content(content).unwrap();

        assert_eq!(elements[0].handle, "intro");
        assert_eq!(elements[1].handle, "titre_1");
    }
}
