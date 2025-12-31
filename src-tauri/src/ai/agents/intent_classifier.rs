use crate::ai::llm::client::{LlmBackend, LlmClient};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Serialize, Deserialize, Clone)]
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
    // CORRECTION 1 : On accepte explicitement "generate_code" ET l'alias "create_code"
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

        // Code Generation (Si explicite)
        if (lower_input.contains("génère") || lower_input.contains("generate"))
            && lower_input.contains("code")
        {
            // On laisse le LLM faire le parsing fin (langage, filename),
            // mais on pourrait court-circuiter ici si on voulait être agressif.
            // Pour l'instant, on laisse passer au LLM ou Fallback.
        }

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

        let system_prompt = "Tu es le Dispatcher IA de GenAptitude.
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

        let clean_json = extract_json(&response);
        let mut json_value: Value = serde_json::from_str(&clean_json).unwrap_or(json!({}));

        // --- MODE SECOURS (HEURISTIQUE) ---
        // Si le LLM échoue à produire un JSON valide avec un "intent"
        if json_value.get("intent").is_none() {
            println!("⚠️  LLM confus, activation du mode heuristique.");
            json_value = heuristic_fallback(user_input);
        }

        // --- CORRECTIONS IMPÉRATIVES (OVERRIDES POST-LLM) ---
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

        // Nom par défaut
        if json_value["intent"] == "create_element" && json_value.get("name").is_none() {
            json_value["name"] = json!(user_input.replace("Crée ", "").replace("le ", "").trim());
        }

        match serde_json::from_value::<EngineeringIntent>(json_value) {
            Ok(intent) => intent,
            Err(e) => {
                println!("❌ Echec total classification : {}", e);
                EngineeringIntent::Unknown
            }
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

    // CORRECTION 2 : Gestion du cas "Générer Code" dans le fallback
    if lower.contains("code") || lower.contains("génère") || lower.contains("generate") {
        return json!({
            "intent": "generate_code",
            "language": "rust", // Valeur par défaut sensée
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

fn extract_json(text: &str) -> String {
    let start_index = match text.find('{') {
        Some(i) => i,
        None => return text.to_string(),
    };
    let end_index = match text.rfind('}') {
        Some(i) => i,
        None => return text[start_index..].to_string(),
    };
    if end_index > start_index {
        return text[start_index..=end_index].trim().to_string();
    }
    text.trim().to_string()
}
