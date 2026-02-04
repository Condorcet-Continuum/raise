// FICHIER : src-tauri/src/model_engine/types.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// --- TYPES FONDAMENTAUX ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum NameType {
    String(String),
    Object(HashMap<String, String>), // Support pour {"en": "...", "fr": "..."}
}

impl NameType {
    pub fn as_str(&self) -> &str {
        match self {
            NameType::String(s) => s,
            NameType::Object(map) => map
                .get("en")
                .or_else(|| map.values().next())
                .map(|s| s.as_str())
                .unwrap_or(""),
        }
    }
}

impl Default for NameType {
    fn default() -> Self {
        NameType::String("".to_string())
    }
}

/// Structure générique représentant n'importe quel élément du modèle Arcadia.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ArcadiaElement {
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub name: NameType,

    /// URI du type (ex: "https://.../oa#OperationalActor") ou nom court ("OperationalActor")
    #[serde(default, rename = "type")]
    pub kind: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Propriétés dynamiques (clé -> valeur/objet)
    #[serde(flatten)]
    pub properties: HashMap<String, serde_json::Value>,
}

impl ArcadiaElement {
    /// Helper pour récupérer le nom affichable
    pub fn get_name(&self) -> &str {
        self.name.as_str()
    }
}

// --- MODÈLE DU PROJET ---

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub last_modified: String,

    // CHAMPS TECHNIQUES RESTAURÉS (Requis par Loader et AuditReport)
    #[serde(default)]
    pub loaded_at: String,
    #[serde(default)]
    pub element_count: usize,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProjectModel {
    pub meta: ProjectMeta,
    pub oa: OperationalAnalysisModel,
    pub sa: SystemAnalysisModel,
    pub la: LogicalArchitectureModel,
    pub pa: PhysicalArchitectureModel,
    pub epbs: EPBSModel,
    pub data: DataModel,
    // AJOUT : Couche Transverse (Exigences, Scénarios, etc.)
    pub transverse: TransverseModel,
}

// --- COUCHES ARCADIA ---

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct OperationalAnalysisModel {
    pub actors: Vec<ArcadiaElement>,
    pub activities: Vec<ArcadiaElement>,
    pub capabilities: Vec<ArcadiaElement>,
    pub entities: Vec<ArcadiaElement>,
    pub exchanges: Vec<ArcadiaElement>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SystemAnalysisModel {
    pub components: Vec<ArcadiaElement>,
    pub functions: Vec<ArcadiaElement>,
    pub actors: Vec<ArcadiaElement>,
    pub capabilities: Vec<ArcadiaElement>,
    pub exchanges: Vec<ArcadiaElement>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LogicalArchitectureModel {
    pub components: Vec<ArcadiaElement>,
    pub functions: Vec<ArcadiaElement>,
    pub actors: Vec<ArcadiaElement>,
    pub interfaces: Vec<ArcadiaElement>,
    pub exchanges: Vec<ArcadiaElement>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PhysicalArchitectureModel {
    pub components: Vec<ArcadiaElement>,
    pub functions: Vec<ArcadiaElement>,
    pub actors: Vec<ArcadiaElement>,
    pub links: Vec<ArcadiaElement>,
    pub exchanges: Vec<ArcadiaElement>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EPBSModel {
    pub configuration_items: Vec<ArcadiaElement>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DataModel {
    pub classes: Vec<ArcadiaElement>,
    pub data_types: Vec<ArcadiaElement>,
    pub exchange_items: Vec<ArcadiaElement>,
}

// AJOUT : Structure pour la couche Transverse
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TransverseModel {
    pub requirements: Vec<ArcadiaElement>,
    pub scenarios: Vec<ArcadiaElement>,
    pub functional_chains: Vec<ArcadiaElement>,
    pub constraints: Vec<ArcadiaElement>,
    pub common_definitions: Vec<ArcadiaElement>,
    pub others: Vec<ArcadiaElement>, // Pour tout ce qui n'est pas catégorisé explicitement ci-dessus
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_name_type_polymorphism() {
        // Cas 1: String simple
        let n1 = NameType::String("Test".to_string());
        assert_eq!(n1.as_str(), "Test");

        // Cas 2: Objet multilingue
        let mut map = HashMap::new();
        map.insert("en".to_string(), "Test EN".to_string());
        map.insert("fr".to_string(), "Test FR".to_string());
        let n2 = NameType::Object(map);
        assert_eq!(n2.as_str(), "Test EN"); // Priorité à l'anglais par défaut
    }

    #[test]
    fn test_arcadia_element_flattening() {
        // Teste que les champs inconnus vont bien dans "properties"
        let json_data = json!({
            "id": "123",
            "name": "MyElement",
            "type": "LogicalComponent",
            "custom_prop": "value",
            "allocated_to": ["456"]
        });

        let el: ArcadiaElement = serde_json::from_value(json_data).unwrap();
        assert_eq!(el.id, "123");
        assert_eq!(el.kind, "LogicalComponent");

        // Vérification des propriétés dynamiques
        assert!(el.properties.contains_key("custom_prop"));
        assert_eq!(el.properties.get("custom_prop").unwrap(), "value");
    }

    #[test]
    fn test_project_model_structure() {
        // Vérifie que la structure contient bien toutes les couches, y compris Transverse
        let model = ProjectModel::default();

        // Les vecteurs doivent être vides par défaut
        assert!(model.oa.actors.is_empty());
        assert!(model.transverse.requirements.is_empty());
        assert!(model.transverse.scenarios.is_empty());
    }

    #[test]
    fn test_project_meta_fields() {
        // Vérifie que les champs restaurés sont bien là et accessibles
        let meta = ProjectMeta {
            loaded_at: "now".to_string(),
            element_count: 42,
            ..Default::default()
        };
        assert_eq!(meta.element_count, 42);
        assert_eq!(meta.loaded_at, "now");
    }

    #[test]
    fn test_transverse_model_serialization() {
        let mut transverse = TransverseModel::default();
        transverse.requirements.push(ArcadiaElement {
            id: "REQ-001".to_string(),
            name: NameType::String("Perf Requirement".to_string()),
            kind: "Requirement".to_string(),
            ..Default::default()
        });

        // Sérialisation
        let json = serde_json::to_string(&transverse).unwrap();
        assert!(json.contains("requirements"));
        assert!(json.contains("REQ-001"));
    }
}
