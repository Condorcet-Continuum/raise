// FICHIER : src-tauri/src/ai/nlp/entity_extractor.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Façade Unique

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
    ArcadiaType,   // Fonction, Composant, Acteur (Dynamique via Ontologie)
}

/// Extrait les entités potentielles d'une phrase en s'appuyant sur le Graphe de Connaissance.
pub async fn extract_entities(manager: &CollectionsManager<'_>, text: &str) -> Vec<Entity> {
    let mut entities = Vec::new();

    // 1. Extraction des textes entre guillemets (Priorité Haute)
    // 🎯 Rigueur : Pattern matching sur la compilation Regex
    let re_quotes = match TextRegex::new(r#"["']([^"']+)["']"#) {
        Ok(re) => re,
        Err(e) => {
            user_error!(
                "ERR_NLP_REGEX_FAIL",
                json_value!({ "error": e.to_string(), "pattern": "quotes" })
            );
            return entities;
        }
    };

    for cap in re_quotes.captures_iter(text) {
        if let Some(matched) = cap.get(1) {
            entities.push(Entity {
                text: matched.as_str().to_string(),
                category: EntityCategory::QuotedLiteral,
            });
        }
    }

    // 2. 🎯 Extraction Dynamique via l'Ontologie (Respect des Mount Points)
    let mut domain_concepts = vec![
        "fonction".to_string(),
        "composant".to_string(),
        "acteur".to_string(),
        "interface".to_string(),
        "échange".to_string(),
    ];

    // Tente de récupérer le mapping ontologique depuis la partition système
    // 🎯 Rigueur : On ne panique jamais sur une absence de doc, on fallback
    match manager.get_document("configs", "ontological_mapping").await {
        Ok(Some(onto_doc)) => {
            if let Some(mapping) = onto_doc
                .get("mappings")
                .or_else(|| onto_doc.get("mapping"))
                .and_then(|v| v.as_object())
            {
                for key in mapping.keys() {
                    domain_concepts.push(key.to_lowercase());
                }
            }
        }
        _ => user_trace!(
            "INF_NLP_ONTOLOGY_FALLBACK",
            json_value!({"reason": "Document mapping introuvable"})
        ),
    }

    let lower_text = text.to_lowercase();
    for concept in &domain_concepts {
        // Fusion des deux conditions avec '&&'
        if lower_text.contains(concept)
            && !entities
                .iter()
                .any(|e| e.text.eq_ignore_ascii_case(concept))
        {
            entities.push(Entity {
                text: concept.clone(),
                category: EntityCategory::ArcadiaType,
            });
        }
    }
    // 3. Extraction heuristique des Noms Propres (Capitalized words)
    let re_proper =
        match TextRegex::new(r"\b[A-ZÀ-ÖØ-Þ][a-zà-öø-ÿ]+\b(?:\s+[A-ZÀ-ÖØ-Þ][a-zà-öø-ÿ]+\b)*")
        {
            Ok(re) => re,
            Err(_) => return entities,
        };

    let determinants = ["Le ", "La ", "Les ", "Un ", "Une ", "Des ", "L'"];

    for cap in re_proper.captures_iter(text) {
        if let Some(matched) = cap.get(0) {
            let mut val = matched.as_str().to_string();

            // Nettoyage des déterminants capturés par erreur
            for det in determinants {
                if val.starts_with(det) {
                    val = val[det.len()..].to_string();
                    break;
                }
            }

            // Évite d'ajouter un nom propre si un concept plus précis (ArcadiaType) a déjà été trouvé
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

// =========================================================================
// TESTS UNITAIRES (Rigueur Façade & Résilience)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    /// Test existant : Vérifie l'extraction entre guillemets
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_extract_quotes() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let input = "Créer le composant 'Moteur Diesel' maintenant.";
        let res = extract_entities(&manager, input).await;
        assert!(res
            .iter()
            .any(|e| e.text == "Moteur Diesel" && e.category == EntityCategory::QuotedLiteral));
        Ok(())
    }

    /// Test existant : Vérifie l'extraction via ontologie
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_extract_arcadia() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let input = "Ajoute une Fonction Système.";
        let res = extract_entities(&manager, input).await;
        assert!(res
            .iter()
            .any(|e| e.category == EntityCategory::ArcadiaType));
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à une base vide (Fallback technique)
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_extract_resilience_empty_db() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.db, "void", "void");

        let input = "L'Acteur principal est le Drone.";
        let res = extract_entities(&manager, input).await;

        // Doit trouver "acteur" via le fallback hardcodé et "Drone" via ProperNoun
        assert!(res.iter().any(|e| e.text == "acteur"));
        assert!(res.iter().any(|e| e.text == "Drone"));
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Respect des Mount Points pour l'ontologie dynamique
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_extract_mount_point_ontology() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 🎯 FIX : Initialiser la collection "configs" avant l'usage
        let generic_schema = "db://_system/_system/schemas/v1/db/generic.schema.json";
        manager.create_collection("configs", generic_schema).await?;

        // Injection d'un nouveau concept dans l'ontologie système
        let onto_doc = json_value!({
            "_id": "ontological_mapping",
            "mappings": { "RadarLonguePortee": {} }
        });
        manager.upsert_document("configs", onto_doc).await?;

        let input = "Analyse le RadarLonguePortee.";
        let res = extract_entities(&manager, input).await;

        assert!(
            res.iter().any(|e| e.text == "radarlongueportee"),
            "Le concept dynamique n'a pas été extrait"
        );
        Ok(())
    }
}
