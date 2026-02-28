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

use self::intent_classifier::EngineeringIntent;
use crate::ai::protocols::acl::AclMessage;

// ✅ Imports standardisés via prelude ou crates explicites
use crate::utils::{
    data,       // Pour la sérialisation JSON
    io,         // Pour les opérations fichiers
    prelude::*, // Contient Result, AppError, serde::*, etc.
    DateTime,
    Utc, // Pour les timestamps
};
use async_trait::async_trait;
use std::fmt;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outgoing_message: Option<AclMessage>,
}

impl AgentResult {
    pub fn text(msg: String) -> Self {
        Self {
            message: msg,
            artifacts: vec![],
            outgoing_message: None,
        }
    }

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

// --- STRUCTURES DE MÉMOIRE ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub role: String,
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

#[async_trait]
pub trait Agent: Send + Sync {
    fn id(&self) -> &'static str;
    async fn process(
        &self,
        ctx: &AgentContext,
        intent: &EngineeringIntent,
    ) -> RaiseResult<Option<AgentResult>>;
}

// --- 🛠️ AGENT TOOLBOX (OPTIMISÉE) ---
pub mod tools {
    use super::*;
    use serde_json::Value;
    // Imports nécessaires pour le Smart Linking centralisé
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
    use crate::utils::config::AppConfig;

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

    pub async fn save_artifact(
        ctx: &AgentContext,
        layer: &str,
        collection: &str,
        doc: &Value,
    ) -> RaiseResult<CreatedArtifact> {
        let Some(doc_id_ref) = doc["id"].as_str() else {
            raise_error!(
                "ERR_ARTIFACT_ID_INVALID",
                error = "L'artefact n'a pas d'ID valide",
                context = serde_json::json!({ "doc_snapshot": doc })
            );
        };
        let doc_id = doc_id_ref.to_string();

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
            io::create_dir_all(parent).await?;
        }

        let content = data::stringify_pretty(doc)?;
        io::write(&full_path, content).await?;

        Ok(CreatedArtifact {
            id: doc_id,
            name,
            layer: layer.to_uppercase(),
            element_type,
            path: relative_path,
        })
    }

    // --- SMART LINKING CENTRALISÉ ---

    /// Recherche un élément par son nom dans toutes les couches (SA, LA, PA, EPBS)
    /// Utilise la configuration globale pour cibler la bonne base de données.
    pub async fn find_element_by_name(ctx: &AgentContext, name: &str) -> Option<Value> {
        // 1. Récupération dynamique de la config
        let config = AppConfig::get();
        // ✅ CORRECTION : Utilisation des nouveaux champs system_domain/db
        let space = &config.system_domain;
        let db_name = &config.system_db;

        let manager = CollectionsManager::new(&ctx.db, space, db_name);
        let query_engine = QueryEngine::new(&manager);

        // Liste des collections à scanner (Ordre de priorité : PA -> LA -> SA)
        let collections = [
            "pa_components",
            "la_components",
            "sa_components",
            "functions",
            "actors",
            "capabilities",
        ];

        for col in collections {
            let mut query = Query::new(col);
            query.filter = Some(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition::eq("name", name.into())],
            });
            query.limit = Some(1);

            // On ignore les erreurs de requête (collection inexistante, etc.)
            if let Ok(result) = query_engine.execute_query(query).await {
                if let Some(doc) = result.documents.first() {
                    return Some(doc.clone());
                }
            }
        }
        None
    }

    pub async fn load_session(ctx: &AgentContext) -> RaiseResult<AgentSession> {
        // On stocke les sessions dans le système pour persistance globale
        let manager = CollectionsManager::new(&ctx.db, "un2", "_system");

        // On s'assure que la collection existe (tolérance aux erreurs)
        let _ = manager.create_collection("agent_sessions", None).await;

        match manager
            .get_document("agent_sessions", &ctx.session_id)
            .await
        {
            Ok(Some(doc_value)) => {
                let session: AgentSession = data::from_value(doc_value)?;
                Ok(session)
            }
            _ => {
                let session = AgentSession::new(&ctx.session_id, &ctx.agent_id);
                save_session(ctx, &session).await?;
                Ok(session)
            }
        }
    }

    pub async fn save_session(ctx: &AgentContext, session: &AgentSession) -> RaiseResult<()> {
        let manager = CollectionsManager::new(&ctx.db, "un2", "_system");
        let json_doc = data::to_value(session)?;
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
