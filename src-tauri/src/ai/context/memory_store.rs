use super::conversation_manager::ConversationSession;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Gère la sauvegarde/chargement des sessions de chat sur disque
pub struct MemoryStore {
    storage_path: PathBuf,
}

impl MemoryStore {
    /// Initialise le store dans un dossier donné (ex: .raise/chats/)
    pub fn new(base_path: &Path) -> Result<Self> {
        if !base_path.exists() {
            fs::create_dir_all(base_path)
                .context("Impossible de créer le dossier de stockage des chats")?;
        }
        Ok(Self {
            storage_path: base_path.to_path_buf(),
        })
    }

    /// Sauvegarde une session
    pub fn save_session(&self, session: &ConversationSession) -> Result<()> {
        let file_path = self.get_path(&session.id);
        let json = serde_json::to_string_pretty(session)?;
        fs::write(file_path, json).context("Échec écriture session chat")?;
        Ok(())
    }

    /// Charge une session existante ou en crée une nouvelle si absente
    pub fn load_or_create(&self, session_id: &str) -> Result<ConversationSession> {
        let file_path = self.get_path(session_id);

        if file_path.exists() {
            let content = fs::read_to_string(file_path)?;
            let session: ConversationSession =
                serde_json::from_str(&content).context("Fichier session corrompu")?;
            Ok(session)
        } else {
            Ok(ConversationSession::new(session_id.to_string()))
        }
    }

    /// Liste toutes les sessions disponibles
    pub fn list_sessions(&self) -> Result<Vec<String>> {
        let mut sessions = Vec::new();
        if self.storage_path.exists() {
            for entry in fs::read_dir(&self.storage_path)? {
                let entry = entry?;
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
