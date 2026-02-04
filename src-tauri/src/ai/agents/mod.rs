// FICHIER : src-tauri/src/ai/agents/mod.rs

pub mod business_agent;
pub mod context;
pub mod data_agent;
pub mod epbs_agent;
pub mod hardware_agent;
pub mod intent_classifier;
pub mod orchestrator_agent;
pub mod software_agent;
pub mod system_agent;
pub mod transverse_agent;

pub use self::context::AgentContext;

use crate::ai::protocols::acl::AclMessage;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

use self::intent_classifier::EngineeringIntent;

/// Représente un élément créé ou modifié par un agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedArtifact {
    pub id: String,
    pub name: String,
    pub layer: String,
    pub element_type: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub message: String,
    pub artifacts: Vec<CreatedArtifact>,

    // NOUVEAU : Canal de communication inter-agents (Optionnel)
    // Si présent, le Dispatcher routera ce message au lieu de répondre à l'utilisateur.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outgoing_message: Option<AclMessage>,
}

impl AgentResult {
    /// Constructeur standard pour une réponse textuelle (fin de chaîne)
    pub fn text(msg: String) -> Self {
        Self {
            message: msg,
            artifacts: vec![],
            // Par défaut, pas de communication sortante
            outgoing_message: None,
        }
    }

    /// Constructeur pour initier une communication avec un autre agent
    pub fn communicate(msg: AclMessage) -> Self {
        Self {
            message: format!("🔄 Communication sortante vers {}", msg.receiver),
            artifacts: vec![],
            outgoing_message: Some(msg),
        }
    }
}

impl fmt::Display for AgentResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

// --- STRUCTURES DE MÉMOIRE (PERSISTANCE) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub role: String, // "user", "assistant", "system"
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

impl AgentMessage {
    pub fn new(role: &str, content: &str) -> Self {
        Self {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub agent_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<AgentMessage>,
    pub summary: Option<String>,
}

impl AgentSession {
    pub fn new(id: &str, agent_id: &str) -> Self {
        Self {
            id: id.to_string(),
            agent_id: agent_id.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            messages: Vec::new(),
            summary: None,
        }
    }

    pub fn add_message(&mut self, role: &str, content: &str) {
        self.messages.push(AgentMessage::new(role, content));
        self.updated_at = Utc::now();
    }
}

// --- TRAIT AGENT ---

#[async_trait]
pub trait Agent: Send + Sync {
    fn id(&self) -> &'static str;

    /// Traitement principal de l'agent.
    /// L'agent est responsable de charger/sauvegarder sa session via `ctx` s'il est stateful.
    async fn process(
        &self,
        ctx: &AgentContext,
        intent: &EngineeringIntent,
    ) -> Result<Option<AgentResult>>;
}

// --- 🛠️ AGENT TOOLBOX ---
pub mod tools {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use serde_json::Value;

    /// Extrait le JSON d'une réponse LLM (nettoie Markdown, préambules, etc.)
    pub fn extract_json_from_llm(response: &str) -> String {
        let text = response.trim();
        let text = text
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

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
        layer: &str,
        collection: &str,
        doc: &Value,
    ) -> Result<CreatedArtifact> {
        let doc_id = doc["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("L'artefact n'a pas d'ID"))?
            .to_string();

        let name = doc["name"].as_str().unwrap_or("Unnamed").to_string();

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

    // --- OUTILS DE PERSISTANCE SESSION ---

    /// Charge ou crée une session pour l'agent courant
    pub async fn load_session(ctx: &AgentContext) -> Result<AgentSession> {
        let manager = CollectionsManager::new(&ctx.db, "un2", "_system");

        // On s'assure que la collection existe
        let _ = manager.create_collection("agent_sessions", None).await;

        match manager
            .get_document("agent_sessions", &ctx.session_id)
            .await
        {
            Ok(Some(doc_value)) => {
                let session: AgentSession = serde_json::from_value(doc_value)?;
                Ok(session)
            }
            _ => {
                let session = AgentSession::new(&ctx.session_id, &ctx.agent_id);
                // On la sauvegarde pour l'initialiser proprement
                save_session(ctx, &session).await?;
                Ok(session)
            }
        }
    }

    /// Sauvegarde l'état actuel de la session
    pub async fn save_session(ctx: &AgentContext, session: &AgentSession) -> Result<()> {
        let manager = CollectionsManager::new(&ctx.db, "un2", "_system");
        let json_doc = serde_json::to_value(session)?;
        manager.upsert_document("agent_sessions", json_doc).await?;
        Ok(())
    }
}

// --- TESTS UNITAIRES (TOOLBOX & ACL) ---
#[cfg(test)]
mod tests {
    use super::tools::*;

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
    fn test_session_struct() {
        use super::AgentSession;
        let mut session = AgentSession::new("sess_1", "agent_1");
        session.add_message("user", "Hello");
        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].role, "user");
    }

    // AJOUT : Test spécifique pour le support ACL dans AgentResult
    #[test]
    fn test_agent_result_acl_support() {
        use super::AgentResult;
        use crate::ai::protocols::acl::{AclMessage, Performative};

        // Cas 1 : Réponse texte classique (Pas de ACL)
        let res_text = AgentResult::text("Hello".to_string());
        assert!(res_text.outgoing_message.is_none());

        // Cas 2 : Communication Agent (ACL présent)
        let msg = AclMessage::new(Performative::Request, "sender", "receiver", "content");
        let res_acl = AgentResult::communicate(msg);

        assert!(res_acl.outgoing_message.is_some());
        assert_eq!(res_acl.outgoing_message.unwrap().receiver, "receiver");
    }
}
