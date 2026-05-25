// FICHIER : src-tauri/src/model_engine/types.rs

use crate::utils::prelude::*;

/// Représentation générique d'un élément dans le graphe (Data-Driven)
#[derive(Debug, Serializable, Deserializable, Clone, Default)]
pub struct ArcadiaElement {
    pub id: String,
    pub name: NameType,
    #[serde(rename = "type")]
    pub kind: String,

    // 🎯 PURE GRAPH : Toutes les propriétés (dont 'description') sont aplaties ici.
    // L'utilisation de UnorderedMap (alias du prelude) garantit la compatibilité Raise.
    #[serde(flatten)]
    pub properties: UnorderedMap<String, JsonValue>,
}

/// Gestion flexible des noms (String simple ou objet complexe JSON-LD)
#[derive(Debug, Serializable, Deserializable, Clone)]
#[serde(untagged)]
pub enum NameType {
    String(String),
    Object(UnorderedMap<String, JsonValue>),
}

impl Default for NameType {
    fn default() -> Self {
        NameType::String("Unnamed".to_string())
    }
}

impl NameType {
    /// Helper pour récupérer une représentation textuelle du nom
    pub fn as_str(&self) -> &str {
        match self {
            NameType::String(s) => s,
            NameType::Object(_) => "ComplexName",
        }
    }
}

/// Métadonnées du projet
#[derive(Debug, Serializable, Deserializable, Clone, Default)]
pub struct ProjectMeta {
    pub name: String,
    pub element_count: usize,
}

/// 🎯 Le Modèle "Pure Graph"
/// Remplace les anciens champs statiques (oa, sa, la, pa...) par une structure dynamique.
#[derive(Debug, Serializable, Deserializable, Clone, Default)]
pub struct ProjectModel {
    pub meta: ProjectMeta,
    /// Structure : Layer (ex: "sa") -> Collection (ex: "functions") -> Liste d'éléments
    pub layers: UnorderedMap<String, UnorderedMap<String, Vec<ArcadiaElement>>>,
}

impl ProjectModel {
    /// Ajoute un élément de manière dynamique dans le graphe
    pub fn add_element(&mut self, layer: &str, collection: &str, el: ArcadiaElement) {
        self.layers
            .entry(layer.to_string())
            .or_default()
            .entry(collection.to_string())
            .or_default()
            .push(el);
    }

    /// Récupère une collection spécifique de manière sécurisée (retourne une slice vide si absente)
    pub fn get_collection(&self, layer: &str, collection: &str) -> &[ArcadiaElement] {
        self.layers
            .get(layer)
            .and_then(|cols| cols.get(collection))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Recherche un élément par son identifiant unique dans l'ensemble des couches
    pub fn find_element(&self, id: &str) -> Option<&ArcadiaElement> {
        self.all_elements().into_iter().find(|el| el.id == id)
    }

    /// Itérateur universel : Récupère tous les éléments du modèle, toutes couches confondues
    pub fn all_elements(&self) -> Vec<&ArcadiaElement> {
        self.layers
            .values()
            .flat_map(|collections| collections.values())
            .flat_map(|vec| vec.iter())
            .collect()
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_element(id: &str, name: &str) -> ArcadiaElement {
        let mut properties = UnorderedMap::new();
        properties.insert("description".to_string(), json_value!("Test content"));

        ArcadiaElement {
            id: id.to_string(),
            name: NameType::String(name.to_string()),
            kind: "https://raise.io/ontology/arcadia/la#LogicalComponent".to_string(),
            properties,
        }
    }

    #[test]
    fn test_dynamic_insertion_and_retrieval() {
        let mut model = ProjectModel::default();
        let el = make_test_element("comp_1", "Radar");

        // Test insertion
        model.add_element("la", "components", el);

        // Test récupération directe
        let collection = model.get_collection("la", "components");
        assert_eq!(collection.len(), 1);
        assert_eq!(collection[0].name.as_str(), "Radar");

        // Vérification que les propriétés sont bien présentes (Pure Graph)
        assert_eq!(
            collection[0]
                .properties
                .get("description")
                .unwrap()
                .as_str()
                .unwrap(),
            "Test content"
        );
    }

    #[test]
    fn test_global_search_find_element() {
        let mut model = ProjectModel::default();
        model.add_element("sa", "functions", make_test_element("f_1", "Func1"));
        model.add_element("oa", "actors", make_test_element("a_1", "Actor1"));

        let found = model.find_element("a_1");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name.as_str(), "Actor1");

        let not_found = model.find_element("missing");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_all_elements_iterator() {
        let mut model = ProjectModel::default();
        model.add_element("layer1", "col1", make_test_element("1", "E1"));
        model.add_element("layer1", "col2", make_test_element("2", "E2"));
        model.add_element("layer2", "col1", make_test_element("3", "E3"));

        let all = model.all_elements();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_empty_collection_safety() {
        let model = ProjectModel::default();
        let empty = model.get_collection("non_existent", "layer");
        assert!(empty.is_empty());
    }
}
