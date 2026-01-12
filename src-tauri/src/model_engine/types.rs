// FICHIER : src-tauri/src/model_engine/types.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// On importe les définitions typées pour le "Nouveau Monde"
use crate::model_engine::arcadia::epbs::ConfigurationItem;
// CORRECTION : Ajout des imports manquants pour la couche DATA
use crate::model_engine::arcadia::data::{Class, DataType, ExchangeItem};
use crate::model_engine::arcadia::logical_architecture::{
    LogicalActor, LogicalComponent, LogicalComponentExchange as LaComponentExchange,
    LogicalFunction, LogicalFunctionalExchange as LaFunctionalExchange, LogicalInterface,
};
use crate::model_engine::arcadia::operational_analysis::{
    OperationalActivity, OperationalActor, OperationalCapability, OperationalEntity,
    OperationalExchange,
};
use crate::model_engine::arcadia::physical_architecture::{
    PhysicalActor, PhysicalComponent, PhysicalComponentExchange as PaComponentExchange,
    PhysicalFunction, PhysicalLink,
};
use crate::model_engine::arcadia::system_analysis::{
    SystemActor, SystemCapability, SystemComponent, SystemFunction,
    SystemFunctionalExchange as SaFunctionalExchange,
};

// =========================================================================
// 1. COMPATIBILITÉ "LEGACY" (Pour xmi_parser, loader, retriever...)
// =========================================================================

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum NameType {
    String(String),
    I18n(HashMap<String, String>),
}

impl Default for NameType {
    fn default() -> Self {
        NameType::String("Sans nom".to_string())
    }
}

impl NameType {
    pub fn as_str(&self) -> &str {
        match self {
            NameType::String(s) => s,
            NameType::I18n(map) => map
                .get("fr")
                .or_else(|| map.get("en"))
                .map(|s| s.as_str())
                .unwrap_or("Sans nom"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArcadiaElement {
    pub id: String,

    #[serde(default)]
    pub name: NameType,

    #[serde(rename = "type")]
    pub kind: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
}

impl ArcadiaElement {
    pub fn new(id: &str, name: &str, kind: &str) -> Self {
        Self {
            id: id.to_string(),
            name: NameType::String(name.to_string()),
            kind: kind.to_string(),
            description: None,
            properties: HashMap::new(),
        }
    }
}

impl Default for ArcadiaElement {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: NameType::default(),
            kind: "Unknown".to_string(),
            description: None,
            properties: HashMap::new(),
        }
    }
}

// =========================================================================
// 2. MODÈLE DE PROJET GÉNÉRIQUE (Pour le Loader actuel)
// =========================================================================

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModel {
    #[serde(default)]
    pub oa: OperationalAnalysisLayer,
    #[serde(default)]
    pub sa: SystemAnalysisLayer,
    #[serde(default)]
    pub la: LogicalArchitectureLayer,
    #[serde(default)]
    pub pa: PhysicalArchitectureLayer,
    #[serde(default)]
    pub epbs: EPBSLayer,
    #[serde(default)]
    pub data: DataLayer,
    #[serde(default)]
    pub meta: ProjectMeta,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeta {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub loaded_at: String,
    #[serde(default)]
    pub element_count: usize,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationalAnalysisLayer {
    #[serde(default)]
    pub actors: Vec<ArcadiaElement>,
    #[serde(default)]
    pub activities: Vec<ArcadiaElement>,
    #[serde(default)]
    pub capabilities: Vec<ArcadiaElement>,
    #[serde(default)]
    pub entities: Vec<ArcadiaElement>,
    #[serde(default)]
    pub exchanges: Vec<ArcadiaElement>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemAnalysisLayer {
    #[serde(default)]
    pub components: Vec<ArcadiaElement>,
    #[serde(default)]
    pub actors: Vec<ArcadiaElement>,
    #[serde(default)]
    pub functions: Vec<ArcadiaElement>,
    #[serde(default)]
    pub capabilities: Vec<ArcadiaElement>,
    #[serde(default)]
    pub exchanges: Vec<ArcadiaElement>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalArchitectureLayer {
    #[serde(default)]
    pub components: Vec<ArcadiaElement>,
    #[serde(default)]
    pub actors: Vec<ArcadiaElement>,
    #[serde(default)]
    pub functions: Vec<ArcadiaElement>,
    #[serde(default)]
    pub interfaces: Vec<ArcadiaElement>,
    #[serde(default)]
    pub exchanges: Vec<ArcadiaElement>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalArchitectureLayer {
    #[serde(default)]
    pub components: Vec<ArcadiaElement>,
    #[serde(default)]
    pub actors: Vec<ArcadiaElement>,
    #[serde(default)]
    pub functions: Vec<ArcadiaElement>,
    #[serde(default)]
    pub links: Vec<ArcadiaElement>,
    #[serde(default)]
    pub exchanges: Vec<ArcadiaElement>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EPBSLayer {
    #[serde(default)]
    pub configuration_items: Vec<ArcadiaElement>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataLayer {
    #[serde(default)]
    pub classes: Vec<ArcadiaElement>,
    #[serde(default)]
    pub data_types: Vec<ArcadiaElement>,
    #[serde(default)]
    pub exchange_items: Vec<ArcadiaElement>,
}

// =========================================================================
// 3. MODÈLE FORTEMENT TYPÉ
// =========================================================================

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypedProjectModel {
    pub oa: TypedOperationalAnalysisLayer,
    pub sa: TypedSystemAnalysisLayer,
    pub la: TypedLogicalArchitectureLayer,
    pub pa: TypedPhysicalArchitectureLayer,
    pub epbs: TypedEPBSLayer,
    pub data: TypedDataLayer, // Ajouté précédemment
    pub meta: ProjectMeta,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TypedOperationalAnalysisLayer {
    pub actors: Vec<OperationalActor>,
    pub activities: Vec<OperationalActivity>,
    pub capabilities: Vec<OperationalCapability>,
    pub entities: Vec<OperationalEntity>,
    pub exchanges: Vec<OperationalExchange>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TypedSystemAnalysisLayer {
    pub components: Vec<SystemComponent>,
    pub actors: Vec<SystemActor>,
    pub functions: Vec<SystemFunction>,
    pub capabilities: Vec<SystemCapability>,
    pub exchanges: Vec<SaFunctionalExchange>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TypedLogicalArchitectureLayer {
    pub components: Vec<LogicalComponent>,
    pub actors: Vec<LogicalActor>,
    pub functions: Vec<LogicalFunction>,
    pub interfaces: Vec<LogicalInterface>,
    pub functional_exchanges: Vec<LaFunctionalExchange>,
    pub component_exchanges: Vec<LaComponentExchange>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TypedPhysicalArchitectureLayer {
    pub components: Vec<PhysicalComponent>,
    pub actors: Vec<PhysicalActor>,
    pub functions: Vec<PhysicalFunction>,
    pub links: Vec<PhysicalLink>,
    pub component_exchanges: Vec<PaComponentExchange>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TypedEPBSLayer {
    pub configuration_items: Vec<ConfigurationItem>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TypedDataLayer {
    pub classes: Vec<Class>,
    pub data_types: Vec<DataType>,
    pub exchange_items: Vec<ExchangeItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_compatibility() {
        let el = ArcadiaElement {
            id: "123".to_string(),
            name: NameType::String("OldStyle".to_string()),
            kind: "Test".to_string(),
            description: Some("Desc".to_string()),
            properties: HashMap::new(),
        };

        assert_eq!(el.id, "123");
        assert_eq!(el.name.as_str(), "OldStyle");
    }

    #[test]
    fn test_typed_instantiation() {
        let _model = TypedProjectModel::default();
    }
}
