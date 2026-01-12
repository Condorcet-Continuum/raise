pub mod business_agent;
pub mod context;
pub mod data_agent;
pub mod epbs_agent;
pub mod hardware_agent;
pub mod intent_classifier;
pub mod software_agent;
pub mod system_agent;
pub mod transverse_agent;

pub use self::context::AgentContext;

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Serialize;
use std::fmt;

use self::intent_classifier::EngineeringIntent;

/// Représente un élément créé ou modifié par un agent
#[derive(Debug, Clone, Serialize)]
pub struct CreatedArtifact {
    pub id: String,
    pub name: String,
    pub layer: String,
    pub element_type: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentResult {
    pub message: String,
    pub artifacts: Vec<CreatedArtifact>,
}

impl AgentResult {
    pub fn text(msg: String) -> Self {
        Self {
            message: msg,
            artifacts: vec![],
        }
    }
}

impl fmt::Display for AgentResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[async_trait]
pub trait Agent: Send + Sync {
    fn id(&self) -> &'static str;
    async fn process(
        &self,
        ctx: &AgentContext,
        intent: &EngineeringIntent,
    ) -> Result<Option<AgentResult>>;
}

// --- 🛠️ AGENT TOOLBOX ---
pub mod tools {
    use super::*;
    use serde_json::Value;

    /// Extrait le JSON d'une réponse LLM (nettoie Markdown, préambules, etc.)
    pub fn extract_json_from_llm(response: &str) -> String {
        let text = response.trim();
        // Nettoyage des balises de code Markdown
        let text = text
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        // Recherche des délimiteurs JSON
        let start = text.find('{').unwrap_or(0);
        let end = text.rfind('}').map(|i| i + 1).unwrap_or(text.len());

        if end > start {
            text[start..end].to_string()
        } else {
            text.to_string()
        }
    }

    /// Sauvegarde standardisée d'un artefact sur le disque
    pub fn save_artifact(
        ctx: &AgentContext,
        layer: &str,      // ex: "sa", "la"
        collection: &str, // ex: "functions", "components"
        doc: &Value,
    ) -> Result<CreatedArtifact> {
        let doc_id = doc["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("L'artefact n'a pas d'ID"))?
            .to_string();

        let name = doc["name"].as_str().unwrap_or("Unnamed").to_string();

        // Fallback intelligent pour le type
        let element_type = doc
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("UnknownElement")
            .to_string();

        let relative_path = format!(
            "un2/{}/collections/{}/{}.json",
            layer.to_lowercase(),
            collection,
            doc_id
        );

        let full_path = ctx.paths.domain_root.join(&relative_path);

        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Impossible de créer le dossier {:?}", parent))?;
        }

        std::fs::write(&full_path, serde_json::to_string_pretty(doc)?)
            .with_context(|| format!("Impossible d'écrire le fichier {:?}", full_path))?;

        Ok(CreatedArtifact {
            id: doc_id,
            name,
            layer: layer.to_uppercase(),
            element_type,
            path: relative_path,
        })
    }
}

// --- TESTS UNITAIRES (TOOLBOX) ---
#[cfg(test)]
mod tests {
    use super::tools::*;
    // CORRECTION : L'import inutile 'serde_json::json' a été supprimé

    #[test]
    fn test_extract_json_clean() {
        let input = r#"{"key": "value"}"#;
        assert_eq!(extract_json_from_llm(input), input);
    }

    #[test]
    fn test_extract_json_markdown() {
        let input = "```json\n{\"key\": \"value\"}\n```";
        assert_eq!(extract_json_from_llm(input), "{\"key\": \"value\"}");
    }

    #[test]
    fn test_extract_json_noisy() {
        let input = "Voici le JSON :\n```json\n{\"key\": \"value\"}\n```\nJ'espère que ça aide.";
        assert_eq!(extract_json_from_llm(input), "{\"key\": \"value\"}");
    }

    #[test]
    fn test_extract_json_no_markdown_noisy() {
        let input = "Sure, here is it: {\"key\": \"value\"} ... end.";
        assert_eq!(extract_json_from_llm(input), "{\"key\": \"value\"}");
    }
}
