// FICHIER : src-tauri/src/model_engine/types.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// On n'importe plus les structures spécifiques d'Arcadia,
// car tout est maintenant géré dynamiquement via ArcadiaElement.

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
/// Remplace toutes les anciennes structures rigides (OperationalActor, SystemComponent, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ArcadiaElement {
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub name: NameType,

    /// URI du type (ex: "https://.../oa#OperationalActor") ou nom court ("OperationalActor")
    #[serde(default, rename = "type", alias = "@type")]
    pub kind: String,

    #[serde(default)]
    pub description: Option<String>,

    /// Contient tous les autres champs (relations, attributs techniques)
    #[serde(flatten)]
    pub properties: HashMap<String, serde_json::Value>,
}

// --- STRUCTURE DU PROJET (CONTENEUR) ---

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProjectModel {
    pub meta: ProjectMeta,
    pub oa: OperationalAnalysisModel,
    pub sa: SystemAnalysisModel,
    pub la: LogicalArchitectureModel,
    pub pa: PhysicalArchitectureModel,
    pub epbs: EPBSModel,
    pub data: DataModel,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    pub loaded_at: String,
    pub element_count: usize,
}

// --- SOUS-MODÈLES (LAYERS) ---
// Note : Tous les vecteurs contiennent désormais des ArcadiaElement génériques.

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
