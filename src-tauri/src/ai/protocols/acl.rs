// src-tauri/src/ai/protocols/acl.rs

use crate::utils::{fmt, prelude::*, DateTime, Utc, Uuid};

/// Les types d'actes communicatifs (Performatifs)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")] // Ex: "REQUEST", "INFORM"
pub enum Performative {
    Request,
    Propose,
    Refuse,
    Agree,
    Inform,
    Confirm,
    Failure,
}
impl fmt::Display for Performative {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
/// Structure du message Agent-to-Agent (A2A)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")] // Convention JSON standard
pub struct AclMessage {
    /// Identifiant unique (UUID v4)
    #[serde(rename = "_id")] // Convention type MongoDB/NoSQL souvent utilisée
    pub id: Uuid,

    /// Horodatage UTC
    pub timestamp: DateTime<Utc>,

    /// L'intention communicative
    pub performative: Performative,

    /// ID de l'émetteur
    pub sender: String,

    /// ID du récepteur
    pub receiver: String,

    /// Contenu (Payload)
    pub content: String,

    /// ID de conversation (Optionnel)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,

    /// Référence au message précédent (Optionnel)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<Uuid>,

    /// Ontologie de référence (Optionnel)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ontology: Option<String>,
}

impl AclMessage {
    pub fn new(performative: Performative, sender: &str, receiver: &str, content: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            performative,
            sender: sender.to_string(),
            receiver: receiver.to_string(),
            content: content.to_string(),
            conversation_id: None,
            reply_to: None,
            ontology: None,
        }
    }

    pub fn reply(original: &AclMessage, performative: Performative, content: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            performative,
            sender: original.receiver.clone(),
            receiver: original.sender.clone(),
            content: content.to_string(),
            conversation_id: original.conversation_id.clone(),
            reply_to: Some(original.id),
            ontology: original.ontology.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::data;
    #[test]
    fn test_acl_creation() {
        let msg = AclMessage::new(Performative::Request, "A", "B", "Test");
        assert_eq!(msg.performative, Performative::Request);
        assert!(!msg.id.is_nil());
    }

    #[test]
    fn test_acl_serialization() {
        let msg = AclMessage::new(Performative::Inform, "A", "B", "Data");
        let json = data::stringify(&msg).unwrap();
        // Vérifie que le champ s'appelle bien "_id" et pas "id"
        assert!(json.contains("_id"));
        assert!(json.contains("performative"));
    }
}
