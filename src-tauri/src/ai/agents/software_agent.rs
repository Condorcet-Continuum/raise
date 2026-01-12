use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use super::intent_classifier::EngineeringIntent;
use super::tools::{extract_json_from_llm, save_artifact};
use super::{Agent, AgentContext, AgentResult, CreatedArtifact};
use crate::ai::llm::client::LlmBackend;
use crate::ai::nlp::entity_extractor;

#[derive(Default)]
pub struct SoftwareAgent;

impl SoftwareAgent {
    pub fn new() -> Self {
        Self {}
    }

    async fn ask_llm(&self, ctx: &AgentContext, system: &str, user: &str) -> Result<String> {
        ctx.llm
            .ask(LlmBackend::LocalLlama, system, user)
            .await
            .map_err(|e| anyhow!("Erreur LLM : {}", e))
    }

    async fn enrich_logical_component(
        &self,
        ctx: &AgentContext,
        name: &str,
        description: &str,
    ) -> Result<serde_json::Value> {
        let entities = entity_extractor::extract_entities(name);
        let mut nlp_hint = String::new();
        if !entities.is_empty() {
            nlp_hint.push_str("\n[VOCABULAIRE]: ");
            for entity in entities {
                nlp_hint.push_str(&format!("{}, ", entity.text));
            }
        }

        let system_prompt = "Tu es un Architecte Logiciel. Génère JSON valide.";
        let user_prompt = format!(
            "Crée Composant LA.\nNom: {}\nDesc: {}\n{}\nJSON: {{ \"name\": \"str\", \"implementation_language\": \"rust|cpp\" }}",
            name, description, nlp_hint
        );

        let response = self.ask_llm(ctx, system_prompt, &user_prompt).await?;
        let clean_json = extract_json_from_llm(&response);

        let mut data: serde_json::Value = serde_json::from_str(&clean_json)
            .unwrap_or(json!({ "name": name, "description": description }));

        data["id"] = json!(Uuid::new_v4().to_string());
        data["layer"] = json!("LA");
        data["type"] = json!("LogicalComponent");
        data["createdAt"] = json!(chrono::Utc::now().to_rfc3339());

        Ok(data)
    }
}

#[async_trait]
impl Agent for SoftwareAgent {
    fn id(&self) -> &'static str {
        "software_engineer"
    }

    async fn process(
        &self,
        ctx: &AgentContext,
        intent: &EngineeringIntent,
    ) -> Result<Option<AgentResult>> {
        match intent {
            EngineeringIntent::CreateElement {
                layer: _,
                element_type,
                name,
            } => {
                let doc = self
                    .enrich_logical_component(ctx, name, &format!("Type: {}", element_type))
                    .await?;

                let artifact = save_artifact(ctx, "la", "components", &doc)?;

                Ok(Some(AgentResult {
                    message: format!("Composant logiciel **{}** modélisé.", name),
                    artifacts: vec![artifact],
                }))
            }
            EngineeringIntent::GenerateCode {
                language,
                context,
                filename,
            } => {
                let user = format!("Code pour: {}\nLangage: {}", context, language);
                let code = self
                    .ask_llm(ctx, "Expert Code. Pas de markdown.", &user)
                    .await?;

                let clean_code = code.replace("```rust", "").replace("```", "");
                let relative_path = format!("src-gen/{}", filename);
                let path = ctx.paths.domain_root.join(&relative_path);

                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, clean_code.trim())?;

                Ok(Some(AgentResult {
                    message: format!("Code source généré dans **{}**.", filename),
                    artifacts: vec![CreatedArtifact {
                        id: filename.clone(),
                        name: filename.clone(),
                        layer: "CODE".to_string(),
                        element_type: "SourceFile".to_string(),
                        path: relative_path,
                    }],
                }))
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_software_agent_id() {
        assert_eq!(SoftwareAgent::new().id(), "software_engineer");
    }
}
