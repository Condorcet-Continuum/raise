use crate::ai::llm::client::{LlmBackend, LlmClient};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "intent", content = "params")]
pub enum EngineeringIntent {
    #[serde(rename = "create_element")]
    CreateElement {
        layer: String,
        element_type: String,
        name: String,
    },
    #[serde(rename = "create_relationship")]
    CreateRelationship {
        source_name: String,
        target_name: String,
        relation_type: String,
    },
    #[serde(rename = "chat")]
    Chat,
    #[serde(rename = "unknown")]
    Unknown,
}

pub struct IntentClassifier {
    llm: LlmClient,
}

impl IntentClassifier {
    pub fn new(llm: LlmClient) -> Self {
        Self { llm }
    }

    pub async fn classify(&self, user_input: &str) -> EngineeringIntent {
        // PROMPT "CHIRURGICAL"
        let system_prompt = r#"
        RÔLE : Classificateur d'intention JSON strict pour ingénierie système.
        CONSIGNE : Analyse la phrase et retourne 1 seul JSON.

        ALGORITHME DE DÉCISION :

        SI la phrase contient un verbe d'action ("réalise", "exécute", "pilote", "contient", "est lié à") :
           ALORS -> "create_relationship"
           IMPORTANT : Ne génère JAMAIS "create_element" ici, même si des types sont mentionnés.

        SINON SI la phrase est un ordre de création ("Crée", "Ajoute", "Nouveau", "Défini") :
           ALORS -> "create_element"

        SINON :
           ALORS -> "chat"

        MAPPING TYPES :
        - Acteur/Activity -> "OA"
        - Fonction/Composant -> "SA"

        EXEMPLES DE RÉFÉRENCE (A SUIVRE) :

        Input: "Crée une activité Voler"
        Output: {"intent":"create_element","params":{"layer":"OA","element_type":"Activity","name":"Voler"}}

        Input: "Le Pilote réalise l'activité Voler"
        Output: {"intent":"create_relationship","params":{"source_name":"Pilote","target_name":"Voler","relation_type":"allocation"}}

        Input: "Le Moteur fournit l'énergie"
        Output: {"intent":"create_relationship","params":{"source_name":"Moteur","target_name":"énergie","relation_type":"exchange"}}
        "#;

        match self
            .llm
            .ask(LlmBackend::LocalLlama, system_prompt, user_input)
            .await
        {
            Ok(raw_response) => {
                println!("🔍 [DEBUG LLM RAW]:\n{}", raw_response);

                let json_str = extract_json(&raw_response);
                let clean_json = json_str.replace(r"\_", "_");

                match serde_json::from_str::<EngineeringIntent>(&clean_json) {
                    Ok(mut intent) => {
                        // Filet de sécurité couches
                        if let EngineeringIntent::CreateElement {
                            layer,
                            element_type,
                            ..
                        } = &mut intent
                        {
                            if layer.contains("<")
                                || layer.is_empty()
                                || (layer != "OA" && layer != "SA")
                            {
                                *layer = match element_type.as_str() {
                                    "Activity" | "Activité" => "OA".to_string(),
                                    "Actor" | "Acteur" => "OA".to_string(),
                                    "Function" | "Fonction" => "SA".to_string(),
                                    "Component" | "Composant" => "SA".to_string(),
                                    _ => "OA".to_string(),
                                };
                            }
                        }
                        intent
                    }
                    Err(e) => {
                        println!("⚠️ Erreur parsing JSON: {}", e);
                        println!("   Chaîne extraite: '{}'", clean_json);
                        EngineeringIntent::Unknown
                    }
                }
            }
            Err(e) => {
                println!("❌ Erreur LLM: {}", e);
                EngineeringIntent::Unknown
            }
        }
    }
}

fn extract_json(text: &str) -> String {
    // 1. Stratégie : Priorité au mot clé "intent"
    let key_patterns = ["\"intent\"", "'intent'"];
    for pattern in key_patterns {
        if let Some(key_idx) = text.find(pattern) {
            // On remonte au '{' précédent
            if let Some(start) = text[..key_idx].rfind('{') {
                // On cherche le '}' correspondant en comptant la balance
                let sub = &text[start..];
                let mut balance = 0;
                for (i, c) in sub.chars().enumerate() {
                    if c == '{' {
                        balance += 1;
                    }
                    if c == '}' {
                        balance -= 1;
                        if balance == 0 {
                            return text[start..=start + i].trim().to_string();
                        }
                    }
                }
            }
        }
    }

    // 2. Fallback Markdown
    if let Some(start) = text.find("```json") {
        if let Some(real_end) = text[start + 7..].find("```") {
            return text[start + 7..start + 7 + real_end].trim().to_string();
        }
    }

    // 3. Fallback Brut
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                return text[start..=end].trim().to_string();
            }
        }
    }

    text.trim().to_string()
}
