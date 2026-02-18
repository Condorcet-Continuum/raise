// FICHIER : src-tauri/src/ai/context/tests.rs

use crate::utils::data::HashMap;

use crate::ai::context::retriever::SimpleRetriever;
// CORRECTION 1 : Ajout de NameType dans les imports
use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel};

// Helper pour créer un élément factice rapidement
fn mock_element(name: &str, desc: &str) -> ArcadiaElement {
    // Note : On n'a plus besoin d'insérer "description" dans les properties
    // car elle a maintenant son propre champ dans la structure.
    let props = HashMap::new();

    ArcadiaElement {
        id: "uuid-test".to_string(),
        // CORRECTION 2 : Enveloppement du nom dans NameType::String
        name: NameType::String(name.to_string()),
        kind: "mock:Type".to_string(),
        // CORRECTION 3 : Initialisation du champ description
        description: Some(desc.to_string()),
        properties: props,
    }
}

#[test]
fn test_retriever_finds_relevant_info() {
    // 1. Préparer un modèle mocké
    let mut model = ProjectModel::default();

    // On ajoute un acteur dans l'OA
    model.oa.actors.push(mock_element(
        "Pilote de Drone",
        "Responsable du vol et de la sécurité.",
    ));

    // On ajoute une fonction dans le SA (qui ne devrait pas être trouvée si on cherche 'vol')
    model.sa.functions.push(mock_element(
        "Alimenter Secteur",
        "Fournit l'énergie au système.",
    ));

    // 2. Instancier le retriever
    let retriever = SimpleRetriever::new(model);

    // 3. Test de recherche
    let query = "Qui est responsable du vol ?";
    let context = retriever.retrieve_context(query);

    println!("Contexte généré : \n{}", context);

    // 4. Assertions
    assert!(
        context.contains("Pilote de Drone"),
        "Le contexte doit contenir l'acteur trouvé"
    );
    assert!(
        context.contains("Responsable du vol"),
        "Le contexte doit contenir la description"
    );
}

#[test]
fn test_retriever_empty_search() {
    let model = ProjectModel::default();
    let retriever = SimpleRetriever::new(model);

    let context = retriever.retrieve_context("Rien à voir");
    assert!(
        context.contains("Aucun élément spécifique"),
        "Doit gérer le cas vide"
    );
}
