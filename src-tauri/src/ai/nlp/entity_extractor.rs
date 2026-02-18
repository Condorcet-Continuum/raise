use crate::utils::Regex;

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
    ArcadiaType,   // Fonction, Composant, Acteur
}

/// Extrait les entités potentielles d'une phrase.
pub fn extract_entities(text: &str) -> Vec<Entity> {
    let mut entities = Vec::new();

    // 1. Extraction des textes entre guillemets (Priorité haute)
    // Regex : capture tout ce qui est entre " " ou ' '
    let re_quotes = Regex::new(r#"["']([^"']+)["']"#).unwrap();
    for cap in re_quotes.captures_iter(text) {
        if let Some(matched) = cap.get(1) {
            entities.push(Entity {
                text: matched.as_str().to_string(),
                category: EntityCategory::QuotedLiteral,
            });
        }
    }

    // 2. Extraction des Types Arcadia connus
    let arcadia_types = vec![
        "fonction",
        "composant",
        "acteur",
        "interface",
        "échange",
        "function",
        "component",
        "actor",
        "exchange",
    ];
    let lower_text = text.to_lowercase();
    for t in arcadia_types {
        if lower_text.contains(t) {
            entities.push(Entity {
                text: t.to_string(),
                category: EntityCategory::ArcadiaType,
            });
        }
    }

    // 3. Extraction heuristique des Noms Propres (Séquences de mots avec Majuscule)
    // Ex: "Station Sol" -> capturé.
    // Regex : Mot Majuscule + (Espace + Mot Majuscule optionnel)*
    let re_proper =
        Regex::new(r"\b[A-ZÀ-ÖØ-Þ][a-zà-öø-ÿ]+\b(?:\s+[A-ZÀ-ÖØ-Þ][a-zà-öø-ÿ]+\b)*").unwrap();

    // Liste de déterminants à nettoyer si capturés par erreur (début de phrase)
    let determinants = ["Le ", "La ", "Les ", "Un ", "Une ", "Des ", "L'"];

    for cap in re_proper.captures_iter(text) {
        if let Some(matched) = cap.get(0) {
            let mut val = matched.as_str().to_string();

            // NETTOYAGE : Si l'entité commence par un déterminant (ex: "La Station"), on le retire.
            // Cela corrige le cas où "La" est pris pour un nom propre car il a une majuscule en début de phrase.
            for det in determinants {
                if val.starts_with(det) {
                    // On coupe le déterminant
                    val = val[det.len()..].to_string();
                    break;
                }
            }

            // On évite d'ajouter si c'est vide, déjà capturé ou doublon
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

    #[test]
    fn test_extract_quotes() {
        let input = "Créer le composant 'Moteur Diesel' maintenant.";
        let res = extract_entities(input);
        assert!(res
            .iter()
            .any(|e| e.text == "Moteur Diesel" && e.category == EntityCategory::QuotedLiteral));
    }

    #[test]
    fn test_extract_arcadia() {
        let input = "Ajoute une Fonction Système.";
        let res = extract_entities(input);
        assert!(res
            .iter()
            .any(|e| e.category == EntityCategory::ArcadiaType));
    }

    #[test]
    fn test_extract_proper_nouns() {
        // "La" a une majuscule -> Le regex brut capture "La Station Sol"
        // Le code de nettoyage doit transformer ça en "Station Sol"
        let input = "La Station Sol communique avec le Drone.";
        let res = extract_entities(input);

        // Debug pour voir ce qu'on a reçu si ça échoue
        println!("Entités trouvées : {:?}", res);

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
