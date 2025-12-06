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
    #[serde(rename = "generate_code")]
    GenerateCode {
        language: String,
        context: String,
        filename: String,
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
        let system_prompt = r#"
        RÔLE : Classificateur d'intention JSON strict pour ingénierie système.
        CONSIGNE : Analyse la phrase et retourne 1 seul JSON.

        ALGORITHME DE DÉCISION :

        1. GÉNÉRATION DE CODE :
           Si l'utilisateur demande explicitement du "Code", un "Script", un "Fichier", ou cite un langage.
           -> "generate_code"

        2. RELATIONS (Action) :
           Si la phrase contient un verbe d'action entre deux concepts ("réalise", "exécute", "pilote").
           -> "create_relationship"

        3. CRÉATION (Modélisation) :
           Si la phrase est un ordre de création d'élément d'architecture ("Crée", "Défini", "Ajoute").
           -> "create_element"
           
        MAPPING TYPES :
        - Acteur/Activity -> "OA"
        - Fonction/Composant -> "SA"

        EXEMPLES :
        Input: "Génère le code Rust pour Superviseur"
        Output: {"intent":"generate_code","params":{"language":"Rust","filename":"Superviseur.rs","context":"..."}}

        Input: "Crée une fonction Démarrer"
        Output: {"intent":"create_element","params":{"layer":"SA","element_type":"Function","name":"Démarrer"}}

        Input: "Le Pilote réalise Démarrer"
        Output: {"intent":"create_relationship","params":{"source_name":"Pilote","target_name":"Démarrer","relation_type":"allocation"}}
        "#;

        match self
            .llm
            .ask(LlmBackend::LocalLlama, system_prompt, user_input)
            .await
        {
            Ok(raw_response) => {
                println!("🔍 [DEBUG LLM RAW]:\n{}", raw_response);

                // Extraction robuste
                let json_str = extract_json(&raw_response);

                // Nettoyage des backslashes parasites
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

/// Extrait le JSON en prenant tout ce qu'il y a entre le premier '{' et le dernier '}'
/// Cette méthode est beaucoup plus robuste aux erreurs de formatage des LLM.
fn extract_json(text: &str) -> String {
    // 1. Trouver le début du JSON (première accolade)
    let start_index = match text.find('{') {
        Some(i) => i,
        None => return text.to_string(), // Pas de JSON trouvé
    };

    // 2. Trouver la fin du JSON (dernière accolade)
    let end_index = match text.rfind('}') {
        Some(i) => i,
        None => return text[start_index..].to_string(), // JSON malfermé ? On tente quand même
    };

    // 3. Extraire le bloc si valide
    if end_index > start_index {
        return text[start_index..=end_index].trim().to_string();
    }

    text.trim().to_string()
}
