use crate::code_generator::models::{DocElement, DocElementType};
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

    /// 🧠 Machine à états pour transformer le Markdown brut en Jumeau Numérique.
    pub fn parse_content(content: &str) -> RaiseResult<Vec<DocElement>> {
        let mut elements: Vec<DocElement> = Vec::new();
        let mut parent_stack: Vec<(u32, String)> = Vec::new(); // (Niveau, Handle)
        let mut current_section: Option<DocElement> = None;
        let mut in_code_block = false;
        let mut code_buffer = String::new();
        let mut current_lang = String::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // 1. GESTION DES BLOCS DE CODE (Mermaid / Rust / etc.)
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
                    code_buffer.clear();
                } else {
                    // Fermeture et création de l'élément technique
                    in_code_block = false;
                    let el_type = if current_lang == "mermaid" {
                        DocElementType::MermaidDiagram
                    } else {
                        DocElementType::CodeBlock
                    };
                    elements.push(DocElement {
                        module_id: None,
                        parent_id: parent_stack.last().map(|(_, h)| h.clone()),
                        element_type: el_type,
                        handle: format!("block_{}_{}", current_lang, elements.len()),
                        title: format!("Source {}", current_lang),
                        heading_level: None,
                        content: code_buffer.clone(),
                        language: current_lang.clone(),
                        elements: Vec::new(),
                        metadata: UnorderedMap::new(),
                    });
                }
                continue;
            }

            if in_code_block {
                code_buffer.push_str(line);
                code_buffer.push('\n');
                continue;
            }

            // 2. GESTION DES TITRES (Nouveaux Parents)
            if trimmed.starts_with('#') {
                let level = trimmed.chars().take_while(|&c| c == '#').count() as u32;
                let title = trimmed.trim_start_matches('#').trim().to_string();
                let handle = title.to_lowercase().replace(' ', "_");

                if let Some(mut sec) = current_section.take() {
                    sec.content = sec.content.trim().to_string();
                    elements.push(sec);
                }

                while let Some((p_level, _)) = parent_stack.last() {
                    if *p_level >= level {
                        parent_stack.pop();
                    } else {
                        break;
                    }
                }

                let parent_id = parent_stack.last().map(|(_, h)| h.clone());
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

            // 3. ACCUMULATION DU TEXTE (Avec gestion de l'intro orpheline)
            if let Some(ref mut sec) = current_section {
                sec.content.push_str(line);
                sec.content.push('\n');
            } else if !trimmed.is_empty() {
                // 🎯 FIX : On restaure la détection de l'introduction
                current_section = Some(DocElement {
                    module_id: None,
                    parent_id: None,
                    element_type: DocElementType::MarkdownSection,
                    handle: "intro".to_string(),
                    title: "Introduction".to_string(),
                    heading_level: Some(1),
                    content: line.to_string() + "\n",
                    language: "markdown".to_string(),
                    elements: Vec::new(),
                    metadata: UnorderedMap::new(),
                });
            }
        }

        if let Some(mut sec) = current_section {
            sec.content = sec.content.trim().to_string();
            elements.push(sec);
        }

        // 4. SYNCHRONISATION IHM (Correction Clippy)
        let mut final_elements = elements.clone();
        for child in &elements {
            if let Some(ref pid) = child.parent_id {
                if let Some(parent) = final_elements.iter_mut().find(|e| &e.handle == pid) {
                    parent.elements.push(child.handle.clone());
                }
            }
        }
        Ok(final_elements)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doc_reconciler_full_hierarchy() {
        // 🎯 FIX : On referme bien le bloc Mermaid et le contenu
        let content = "# Racine\n## A\n### A1\n```mermaid\ngraph TD; A-->B;\n```\n";
        let elements = DocReconciler::parse_content(content).expect("Parsing failed");
        assert!(elements.len() >= 4);
        let section_a = elements.iter().find(|e| e.handle == "a").unwrap();
        assert!(section_a.elements.contains(&"a1".to_string()));
    }

    #[test]
    fn test_doc_reconciler_orphan_intro() {
        let content = "Introduction sans titre.\n# Titre 1";
        let elements = DocReconciler::parse_content(content).unwrap();
        // 🎯 FIX : L'ordre est maintenant correct
        assert_eq!(elements[0].handle, "intro");
        assert_eq!(elements[1].handle, "titre_1");
    }
}
