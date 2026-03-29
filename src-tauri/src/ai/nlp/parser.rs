// FICHIER : src-tauri/src/ai/nlp/parser.rs

use crate::ai::nlp::{preprocessing, tokenizers};
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

#[derive(Debug, PartialEq, Clone)]
pub enum CommandType {
    Create,
    Delete,
    Search,
    Explain,
    Unknown,
}

/// Tente de deviner l'intention brute par mots-clés (Rule-Based).
/// Utile pour un "Fast Path" avant d'appeler le LLM.
/// 🎯 FIX : Devenu async pour lire dynamiquement les synonymes d'actions depuis JSON-DB.
pub async fn simple_intent_detection(manager: &CollectionsManager<'_>, text: &str) -> CommandType {
    let normalized = preprocessing::normalize(text);
    let tokens = tokenizers::tokenize(&normalized);

    // 1. Dictionnaires par défaut (Fallback si la DB est vierge)
    let mut create_kw = vec!["creer".to_string(), "ajout".to_string(), "nouv".to_string()];
    let mut delete_kw = vec![
        "supprim".to_string(),
        "retir".to_string(),
        "effac".to_string(),
    ];
    let mut search_kw = vec![
        "cherch".to_string(),
        "trouv".to_string(),
        "list".to_string(),
    ];
    let mut explain_kw = vec![
        "expliqu".to_string(),
        "comment".to_string(),
        "quois".to_string(),
    ];

    // 2. 🎯 Hydratation dynamique via le Graphe de Connaissance
    if let Ok(Some(action_doc)) = manager.get_document("configs", "action_mapping").await {
        if let Some(mapping) = action_doc.get("actions").and_then(|v| v.as_object()) {
            let extract_kws = |key: &str, target: &mut Vec<String>| {
                if let Some(arr) = mapping.get(key).and_then(|v| v.as_array()) {
                    target.extend(arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())));
                }
            };

            extract_kws("create", &mut create_kw);
            extract_kws("delete", &mut delete_kw);
            extract_kws("search", &mut search_kw);
            extract_kws("explain", &mut explain_kw);
        }
    }

    // 3. Détection par intersection
    if tokens
        .iter()
        .any(|t| create_kw.iter().any(|kw| t.contains(kw)))
    {
        return CommandType::Create;
    }
    if tokens
        .iter()
        .any(|t| delete_kw.iter().any(|kw| t.contains(kw)))
    {
        return CommandType::Delete;
    }
    if tokens
        .iter()
        .any(|t| search_kw.iter().any(|kw| t.contains(kw)))
    {
        return CommandType::Search;
    }
    if tokens
        .iter()
        .any(|t| explain_kw.iter().any(|kw| t.contains(kw)))
    {
        return CommandType::Explain;
    }

    CommandType::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_detect_create() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        assert_eq!(
            simple_intent_detection(&manager, "Je veux créer une fonction").await,
            CommandType::Create
        );
        assert_eq!(
            simple_intent_detection(&manager, "Ajoute un composant").await,
            CommandType::Create
        );
    }

    #[async_test]
    async fn test_detect_explain() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        assert_eq!(
            simple_intent_detection(&manager, "Explique-moi Arcadia").await,
            CommandType::Explain
        );
    }
}
