// FICHIER : src-tauri/src/ai/nlp/parser.rs

use crate::ai::nlp::{preprocessing, tokenizers};
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Façade Unique

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
/// Lit dynamiquement les synonymes d'actions depuis la partition Système du Graphe de Connaissance.
pub async fn simple_intent_detection(manager: &CollectionsManager<'_>, text: &str) -> CommandType {
    let normalized = preprocessing::normalize(text);
    let tokens = tokenizers::tokenize(&normalized);

    // 1. Dictionnaires par défaut (Fallback technique si l'ontologie est inaccessible)
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

    // 2. 🎯 Hydratation dynamique via l'Ontologie (Respect des Mount Points)
    // 🎯 Rigueur : On ne panique jamais sur une absence de doc, on fallback sur les mots-clés statiques.
    match manager.get_document("configs", "action_mapping").await {
        Ok(Some(action_doc)) => {
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
        _ => user_trace!(
            "INF_NLP_PARSER_ONTOLOGY_OFF",
            json_value!({"reason": "action_mapping introuvable"})
        ),
    }

    // 3. Détection d'intention par intersection (Priorité ordonnée)
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

// =========================================================================
// TESTS UNITAIRES (Rigueur Façade & Résilience)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    /// Test existant : Détection de création
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_detect_create() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        assert_eq!(
            simple_intent_detection(&manager, "Je veux créer une fonction").await,
            CommandType::Create
        );
        assert_eq!(
            simple_intent_detection(&manager, "Ajoute un composant").await,
            CommandType::Create
        );
        Ok(())
    }

    /// Test existant : Détection d'explication
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_detect_explain() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        assert_eq!(
            simple_intent_detection(&manager, "Explique-moi Arcadia").await,
            CommandType::Explain
        );
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à une base vide (Fallback technique)
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_parser_resilience_empty_db() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        // Manager pointant sur un domaine vide
        let manager = CollectionsManager::new(&sandbox.db, "void", "void");

        // Doit toujours détecter "chercher" via le dictionnaire statique
        assert_eq!(
            simple_intent_detection(&manager, "Peux-tu chercher le drone ?").await,
            CommandType::Search
        );
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Respect des Mount Points pour les nouveaux synonymes
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_parser_mount_point_custom_synonyms() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 🎯 FIX : Initialiser la collection "configs" avant l'usage
        let generic_schema = "db://_system/_system/schemas/v1/db/generic.schema.json";
        manager.create_collection("configs", generic_schema).await?;

        // Injection d'un synonyme spécifique dans la partition système
        let action_doc = json_value!({
            "_id": "action_mapping",
            "actions": {
                "explain": ["clarifie", "detaille"]
            }
        });
        manager.upsert_document("configs", action_doc).await?;

        // Le parser doit maintenant reconnaître "clarifie" comme une intention "Explain"
        assert_eq!(
            simple_intent_detection(&manager, "Clarifie ce concept.").await,
            CommandType::Explain
        );
        Ok(())
    }
}
