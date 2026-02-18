use crate::utils::{prelude::*, DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// Gère une session de chat active
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSession {
    pub id: String,
    pub history: Vec<ChatMessage>,
    /// Nombre max de messages à conserver en mémoire vive (fenêtre glissante)
    pub max_history_len: usize,
}

impl ConversationSession {
    pub fn new(id: String) -> Self {
        Self {
            id,
            history: Vec::new(),
            max_history_len: 10, // Valeur par défaut raisonnable
        }
    }

    /// Ajoute un message utilisateur
    pub fn add_user_message(&mut self, content: &str) {
        self.push_message(Role::User, content);
    }

    /// Ajoute une réponse de l'IA
    pub fn add_ai_message(&mut self, content: &str) {
        self.push_message(Role::Assistant, content);
    }

    fn push_message(&mut self, role: Role, content: &str) {
        self.history.push(ChatMessage {
            role,
            content: content.to_string(),
            timestamp: Utc::now(),
        });
        self.enforce_limit();
    }

    /// Convertit l'historique en texte formaté pour le Prompt du LLM
    pub fn to_context_string(&self) -> String {
        if self.history.is_empty() {
            return String::new();
        }

        let mut buffer = String::from("### HISTORIQUE DE CONVERSATION ###\n");
        for msg in &self.history {
            let prefix = match msg.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
                Role::System => "System",
            };
            buffer.push_str(&format!("{}: {}\n", prefix, msg.content));
        }
        buffer.push_str("##################################\n\n");
        buffer
    }

    /// Supprime les vieux messages si on dépasse la limite
    fn enforce_limit(&mut self) {
        if self.history.len() > self.max_history_len {
            let remove_count = self.history.len() - self.max_history_len;
            self.history.drain(0..remove_count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_formatting() {
        let mut session = ConversationSession::new("test".to_string());
        session.add_user_message("Bonjour");
        session.add_ai_message("Salut");

        let ctx = session.to_context_string();
        assert!(ctx.contains("User: Bonjour"));
        assert!(ctx.contains("Assistant: Salut"));
    }

    #[test]
    fn test_sliding_window() {
        // On force une limite de 2 messages
        let mut session = ConversationSession::new("test".to_string());
        session.max_history_len = 2;

        session.add_user_message("1");
        session.add_ai_message("2");
        session.add_user_message("3"); // Devrait éjecter "1"

        assert_eq!(session.history.len(), 2);
        assert_eq!(session.history[0].content, "2");
        assert_eq!(session.history[1].content, "3");
    }
}
