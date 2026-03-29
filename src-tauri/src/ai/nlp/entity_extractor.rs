// FICHIER : src-tauri/src/ai/nlp/entity_extractor.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

/// Structure représentant une entité extraite du texte.
#[derive(Debug, PartialEq, Clone)]
pub struct Entity {
    pub text: String,
    pub category: EntityCategory,
}

#[derive(Debug, PartialEq, Clone)]
pub enum EntityCategory {
    QuotedLiteral, // "Mon Système"
    ProperNoun,    // Moteur, Station Sol (Mots avec majuscules)
    ArcadiaType,   // Fonction, Composant, Acteur (Dynamique via DB)
}

/// Extrait les entités potentielles d'une phrase.
/// 🎯 FIX : Devenu async pour interroger le Graphe de Connaissance
pub async fn extract_entities(manager: &CollectionsManager<'_>, text: &str) -> Vec<Entity> {
    let mut entities = Vec::new();

    // 1. Extraction des textes entre guillemets (Priorité haute)
    // Regex : capture tout ce qui est entre " " ou ' '
    let re_quotes = TextRegex::new(r#"["']([^"']+)["']"#).unwrap();
    for cap in re_quotes.captures_iter(text) {
        if let Some(matched) = cap.get(1) {
            entities.push(Entity {
                text: matched.as_str().to_string(),
                category: EntityCategory::QuotedLiteral,
            });
        }
    }

    // 2. 🎯 Extraction Dynamique via JSON-DB (Ontologie)
    // On garde un fallback de base au cas où la DB serait vierge
    let mut domain_concepts = vec![
        "fonction".to_string(),
        "composant".to_string(),
        "acteur".to_string(),
        "interface".to_string(),
        "échange".to_string(),
        "function".to_string(),
        "component".to_string(),
        "actor".to_string(),
        "exchange".to_string(),
    ];

    // Tente de récupérer le mapping ontologique depuis la DB
    if let Ok(Some(onto_doc)) = manager.get_document("configs", "ontological_mapping").await {
        if let Some(mapping) = onto_doc.get("mapping").and_then(|v| v.as_object()) {
            for key in mapping.keys() {
                domain_concepts.push(key.to_lowercase());
            }
        }
    }

    let lower_text = text.to_lowercase();
    for t in &domain_concepts {
        if lower_text.contains(t) {
            // Évite les doublons exacts
            if !entities.iter().any(|e| e.text.eq_ignore_ascii_case(t)) {
                entities.push(Entity {
                    text: t.clone(),
                    category: EntityCategory::ArcadiaType,
                });
            }
        }
    }

    // 3. Extraction heuristique des Noms Propres (Séquences de mots avec Majuscule)
    let re_proper =
        TextRegex::new(r"\b[A-ZÀ-ÖØ-Þ][a-zà-öø-ÿ]+\b(?:\s+[A-ZÀ-ÖØ-Þ][a-zà-öø-ÿ]+\b)*").unwrap();

    let determinants = ["Le ", "La ", "Les ", "Un ", "Une ", "Des ", "L'"];

    for cap in re_proper.captures_iter(text) {
        if let Some(matched) = cap.get(0) {
            let mut val = matched.as_str().to_string();

            for det in determinants {
                if val.starts_with(det) {
                    val = val[det.len()..].to_string();
                    break;
                }
            }

            if !val.is_empty() && !entities.iter().any(|e| e.text.contains(&val)) {
                entities.push(Entity {
                    text: val,
                    category: EntityCategory::ProperNoun,
                });
            }
        }
    }

    entities
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_extract_quotes() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let input = "Créer le composant 'Moteur Diesel' maintenant.";
        let res = extract_entities(&manager, input).await;
        assert!(res
            .iter()
            .any(|e| e.text == "Moteur Diesel" && e.category == EntityCategory::QuotedLiteral));
    }

    #[async_test]
    async fn test_extract_arcadia() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let input = "Ajoute une Fonction Système.";
        let res = extract_entities(&manager, input).await;
        assert!(res
            .iter()
            .any(|e| e.category == EntityCategory::ArcadiaType));
    }

    #[async_test]
    async fn test_extract_proper_nouns() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let input = "La Station Sol communique avec le Drone.";
        let res = extract_entities(&manager, input).await;

        assert!(
            res.iter().any(|e| e.text == "Station Sol"),
            "Station Sol devrait être détecté sans 'La'"
        );
        assert!(
            res.iter().any(|e| e.text == "Drone"),
            "Drone devrait être détecté"
        );
    }
}
