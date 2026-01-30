// FICHIER : src-tauri/src/ai/agents/intent_classifier.rs

use crate::ai::llm::client::{LlmBackend, LlmClient};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// Import de la Toolbox pour le parsing JSON robuste
use super::tools::extract_json_from_llm;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "intent")]
pub enum EngineeringIntent {
    #[serde(rename = "define_business_use_case")]
    DefineBusinessUseCase {
        domain: String,
        process_name: String,
        description: String,
    },
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
    #[serde(rename = "generate_code", alias = "create_code")]
    GenerateCode {
        language: String,
        #[serde(alias = "content", alias = "code", default)]
        context: String,
        filename: String,
    },
    #[serde(rename = "chat")]
    Chat,
    #[serde(rename = "unknown")]
    Unknown,
}

impl EngineeringIntent {
    /// Retourne l'ID de l'agent le plus qualifié pour traiter cette intention.
    /// Centralise la logique de routage du système.
    pub fn recommended_agent_id(&self) -> &'static str {
        match self {
            Self::DefineBusinessUseCase { .. } => "business_agent",
            Self::CreateElement { layer, .. } => match layer.as_str() {
                "OA" => "business_agent",
                "SA" => "system_agent",
                "LA" => "software_agent",
                "PA" => "hardware_agent",
                "EPBS" => "epbs_agent",
                "DATA" => "data_agent",
                "TRANSVERSE" => "transverse_agent",
                _ => "orchestrator_agent", // Fallback
            },
            Self::CreateRelationship { .. } => "system_agent", // Par défaut, souvent géré au niveau système
            Self::GenerateCode { .. } => "software_agent",
            Self::Chat | Self::Unknown => "orchestrator_agent",
        }
    }

    /// Définit le scope de session par défaut pour cette intention.
    /// Utile pour savoir si on doit reprendre le contexte global ou créer une branche.
    pub fn default_session_scope(&self) -> &'static str {
        match self {
            Self::Chat => "global_chat",
            _ => "main_workflow",
        }
    }
}

pub struct IntentClassifier {
    llm: LlmClient,
}

impl IntentClassifier {
    pub fn new(llm: LlmClient) -> Self {
        Self { llm }
    }

    pub async fn classify(&self, user_input: &str) -> EngineeringIntent {
        let lower_input = user_input.to_lowercase();

        // --- 1. COURT-CIRCUIT (Optimisation CPU & Déterminisme) ---
        // On évite le LLM si l'intention est évidente via des mots-clés forts.

        // TRANSVERSE (Exigences, Tests)
        if lower_input.contains("exigence") || lower_input.contains("requirement") {
            return EngineeringIntent::CreateElement {
                layer: "TRANSVERSE".to_string(),
                element_type: "Requirement".to_string(),
                name: extract_name(user_input, "exigence"),
            };
        }
        if lower_input.contains("procédure")
            || (lower_input.contains("test") && lower_input.contains("procedure"))
        {
            return EngineeringIntent::CreateElement {
                layer: "TRANSVERSE".to_string(),
                element_type: "TestProcedure".to_string(),
                name: extract_name(user_input, "procédure"),
            };
        }
        if lower_input.contains("campagne") || lower_input.contains("campaign") {
            return EngineeringIntent::CreateElement {
                layer: "TRANSVERSE".to_string(),
                element_type: "TestCampaign".to_string(),
                name: extract_name(user_input, "campagne"),
            };
        }
        if lower_input.contains("scénario") || lower_input.contains("scenario") {
            return EngineeringIntent::CreateElement {
                layer: "TRANSVERSE".to_string(),
                element_type: "ExchangeScenario".to_string(),
                name: extract_name(user_input, "scénario"),
            };
        }

        // DATA (Classes)
        if lower_input.contains("classe") || lower_input.contains("class") {
            return EngineeringIntent::CreateElement {
                layer: "DATA".to_string(),
                element_type: "Class".to_string(),
                name: extract_name(user_input, "classe"),
            };
        }

        // OA (Capacités)
        if lower_input.contains("capacité") || lower_input.contains("capability") {
            return EngineeringIntent::CreateElement {
                layer: "OA".to_string(),
                element_type: "OperationalCapability".to_string(),
                name: extract_name(user_input, "capacité"),
            };
        }

        // --- 2. APPEL LLM (Fallback Intelligent) ---

        let system_prompt = "Tu es le Dispatcher IA de RAISE.
        Ton rôle est de classifier l'intention de l'utilisateur en JSON STRICT.
        
        FORMATS ATTENDUS :
        1. Création : { \"intent\": \"create_element\", \"layer\": \"OA|SA|LA|PA|DATA|TRANSVERSE\", \"element_type\": \"Type\", \"name\": \"Nom\" }
        2. Code : { \"intent\": \"generate_code\", \"language\": \"rust|python\", \"filename\": \"main.rs\", \"context\": \"description\" }

        Exemple: 'Génère le code Rust pour Auth' -> { \"intent\": \"generate_code\", \"language\": \"rust\", \"filename\": \"auth.rs\", \"context\": \"Auth\" }";

        let response = self
            .llm
            .ask(LlmBackend::LocalLlama, system_prompt, user_input)
            .await
            .unwrap_or_else(|_| "{}".to_string());

        // UTILISATION DE LA TOOLBOX ICI (Plus de code dupliqué)
        let clean_json = extract_json_from_llm(&response);
        let mut json_value: Value = serde_json::from_str(&clean_json).unwrap_or(json!({}));

        // --- MODE SECOURS (HEURISTIQUE) ---
        // Si le LLM échoue à produire un JSON valide avec un "intent"
        if json_value.get("intent").is_none() {
            // println!("⚠️  LLM confus, activation du mode heuristique."); // Log optionnel
            json_value = heuristic_fallback(user_input);
        }

        // --- CORRECTIONS IMPÉRATIVES (OVERRIDES POST-LLM) ---
        // Corrige les erreurs fréquentes du LLM sur les couches
        if let Some(intent) = json_value["intent"].as_str() {
            if intent == "create_element" || intent == "create_system" {
                if lower_input.contains("exigence") || lower_input.contains("requirement") {
                    json_value["layer"] = json!("TRANSVERSE");
                    json_value["element_type"] = json!("Requirement");
                } else if lower_input.contains("classe")
                    || lower_input.contains("donnée")
                    || lower_input.contains("datatype")
                {
                    json_value["layer"] = json!("DATA");
                    json_value["element_type"] = json!("Class");
                } else if lower_input.contains("acteur") || lower_input.contains("rôle") {
                    json_value["layer"] = json!("OA");
                    json_value["element_type"] = json!("OperationalActor");
                } else if lower_input.contains("configuration") || lower_input.contains("article") {
                    json_value["layer"] = json!("EPBS");
                    json_value["element_type"] = json!("ConfigurationItem");
                }
            }
        }

        // Fix legacy intents
        if json_value["intent"] == "create_system" {
            json_value["intent"] = json!("create_element");
            if json_value.get("layer").is_none() {
                json_value["layer"] = json!("SA");
            }
            if json_value.get("element_type").is_none() {
                json_value["element_type"] = json!("System");
            }
        }

        // Nom par défaut si manquant
        if json_value["intent"] == "create_element" && json_value.get("name").is_none() {
            json_value["name"] = json!(user_input.replace("Crée ", "").replace("le ", "").trim());
        }

        match serde_json::from_value::<EngineeringIntent>(json_value) {
            Ok(intent) => intent,
            Err(_) => EngineeringIntent::Unknown,
        }
    }
}

// --- HELPER FUNCTIONS ---

fn extract_name(input: &str, keyword: &str) -> String {
    let lower = input.to_lowercase();
    if let Some(idx) = lower.find(keyword) {
        let raw = &input[idx + keyword.len()..].trim();
        let clean = raw
            .trim_start_matches("de ")
            .trim_start_matches("du ")
            .trim_start_matches("la ")
            .trim_start_matches("le ")
            .trim_start_matches("l'")
            .trim_start_matches("une ")
            .trim_start_matches("un ")
            .trim();
        return clean.to_string();
    }
    input.to_string()
}

fn heuristic_fallback(input: &str) -> Value {
    let lower = input.to_lowercase();

    if lower.contains("code") || lower.contains("génère") || lower.contains("generate") {
        return json!({
            "intent": "generate_code",
            "language": "rust",
            "filename": "generated_component.rs",
            "context": input
        });
    }

    let (layer, etype) = if lower.contains("système") {
        ("SA", "System")
    } else if lower.contains("exigence") {
        ("TRANSVERSE", "Requirement")
    } else if lower.contains("classe") {
        ("DATA", "Class")
    } else if lower.contains("logiciel") {
        ("LA", "Component")
    } else if lower.contains("matériel") {
        ("PA", "PhysicalNode")
    } else if lower.contains("acteur") {
        ("OA", "OperationalActor")
    } else {
        ("SA", "Function")
    };

    json!({ "intent": "create_element", "layer": layer, "element_type": etype, "name": input })
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recommended_agent_routing() {
        let intent_sa = EngineeringIntent::CreateElement {
            layer: "SA".to_string(),
            element_type: "System".to_string(),
            name: "Test".to_string(),
        };
        assert_eq!(intent_sa.recommended_agent_id(), "system_agent");

        let intent_la = EngineeringIntent::CreateElement {
            layer: "LA".to_string(),
            element_type: "Component".to_string(),
            name: "Test".to_string(),
        };
        assert_eq!(intent_la.recommended_agent_id(), "software_agent");

        let intent_code = EngineeringIntent::GenerateCode {
            language: "rust".into(),
            context: "".into(),
            filename: "".into(),
        };
        assert_eq!(intent_code.recommended_agent_id(), "software_agent");
    }

    #[test]
    fn test_extract_name() {
        assert_eq!(
            extract_name("Crée une exigence de performance", "exigence"),
            "performance"
        );
        assert_eq!(
            extract_name("Crée l'exigence l'autonomie", "exigence"),
            "autonomie"
        );
        assert_eq!(
            extract_name("Nouvelle classe utilisateur", "classe"),
            "utilisateur"
        );
    }

    #[test]
    fn test_heuristic_fallback_code() {
        let val = heuristic_fallback("Génère code python");
        assert_eq!(val["intent"], "generate_code");
        assert_eq!(val["language"], "rust"); // Default hardcodé dans fallback
    }

    #[test]
    fn test_heuristic_fallback_create() {
        let val = heuristic_fallback("Ajoute un composant logiciel");
        assert_eq!(val["intent"], "create_element");
        assert_eq!(val["layer"], "LA");
        assert_eq!(val["element_type"], "Component");
    }
}
