use crate::utils::{
    data,
    io::{self, PathBuf},
    prelude::*,
};

use super::conversation_manager::ConversationSession;

/// Gère la sauvegarde/chargement des sessions de chat sur disque
pub struct MemoryStore {
    storage_path: PathBuf,
}

impl MemoryStore {
    /// Initialise le store dans un dossier donné (ex: .raise/chats/)
    pub async fn new(base_path: &Path) -> RaiseResult<Self> {
        if !base_path.exists() {
            io::create_dir_all(base_path).await.map_err(|e| {
                AppError::custom_io(format!("Impossible de créer le dossier des chats : {}", e))
            })?;
        }
        Ok(Self {
            storage_path: base_path.to_path_buf(),
        })
    }

    /// Sauvegarde une session
    pub async fn save_session(&self, session: &ConversationSession) -> RaiseResult<()> {
        let file_path = self.get_path(&session.id);
        let json = data::stringify_pretty(session)?;
        io::write(file_path, json)
            .await
            .map_err(|e| AppError::custom_io(format!("Échec écriture session : {}", e)))?;
        Ok(())
    }

    /// Charge une session existante ou en crée une nouvelle si absente
    pub async fn load_or_create(&self, session_id: &str) -> RaiseResult<ConversationSession> {
        let file_path = self.get_path(session_id);

        if file_path.exists() {
            let content = io::read_to_string(&file_path).await?;
            let session: ConversationSession = data::parse(&content)?;
            Ok(session)
        } else {
            Ok(ConversationSession::new(session_id.to_string()))
        }
    }

    /// Liste toutes les sessions disponibles
    pub async fn list_sessions(&self) -> RaiseResult<Vec<String>> {
        let mut sessions = Vec::new();
        if self.storage_path.exists() {
            let mut dir = io::read_dir(&self.storage_path).await.map_err(|e| {
                AppError::custom_io(format!("Impossible de lire le dossier : {}", e))
            })?;
            while let Ok(Some(entry)) = dir.next_entry().await {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        sessions.push(stem.to_string());
                    }
                }
            }
        }
        Ok(sessions)
    }

    fn get_path(&self, session_id: &str) -> PathBuf {
        self.storage_path.join(format!("{}.json", session_id))
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
