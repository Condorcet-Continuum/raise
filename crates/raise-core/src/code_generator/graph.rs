// FICHIER : src-tauri/src/code_generator/graph.rs

use crate::code_generator::models::CodeElement;
use crate::utils::prelude::*;

/// 🔄 États pour la simulation de la pile d'appels récursive
enum DfsState {
    Processing(String), // Première visite : on empile les dépendances
    Processed(String),  // Visite de retour : dépendances résolues, on valide
}

/// 🧠 Ordonne les éléments de code pour garantir une compilation sans erreur.
/// Algorithme : Tri topologique par DFS itératif (zéro risque de Stack Overflow).
pub fn sort_elements_topologically(elements: Vec<CodeElement>) -> RaiseResult<Vec<CodeElement>> {
    let mut sorted = Vec::with_capacity(elements.len());
    let mut visited = UniqueSet::new(); // Éléments totalement traités
    let mut visiting = UniqueSet::new(); // Éléments en cours (pour détection de cycle)

    // Indexation par handle pour un accès rapide (O(1)) via la façade RAISE
    let mut elements_map: UnorderedMap<String, CodeElement> = elements
        .into_iter()
        .map(|e| (e.handle.clone(), e))
        .collect();

    let handles: Vec<String> = elements_map.keys().cloned().collect();

    // Notre pile d'appels explicite allouée sur le tas
    let mut stack = Vec::new();

    for root_handle in handles {
        if visited.contains(&root_handle) {
            continue; // Déjà traité par une autre branche
        }

        stack.push(DfsState::Processing(root_handle));

        while let Some(state) = stack.pop() {
            match state {
                DfsState::Processing(handle) => {
                    if visited.contains(&handle) {
                        continue;
                    }

                    // 1. Détection de cycle (Ligne rouge sémantique)
                    if visiting.contains(&handle) {
                        raise_error!(
                            "ERR_CODEGEN_CIRCULAR_DEPENDENCY",
                            error = format!("Cycle détecté impliquant l'élément : {}", handle),
                            context = json_value!({ "handle": handle })
                        );
                    }

                    // 2. Marquage "en cours d'exploration"
                    visiting.insert(handle.clone());

                    // 3. Empiler la phase de retour (Processed) POUR CE NŒUD
                    // Elle sera dépilée APRES toutes ses dépendances
                    stack.push(DfsState::Processed(handle.clone()));

                    // 4. Exploration des dépendances
                    if let Some(element) = elements_map.get(&handle) {
                        // On utilise .rev() pour préserver l'ordre d'exploration exact
                        // de l'ancienne version récursive (LIFO : le dernier empilé sera le premier dépilé)
                        for dep_handle in element.dependencies.iter().rev() {
                            if elements_map.contains_key(dep_handle) {
                                stack.push(DfsState::Processing(dep_handle.clone()));
                            }
                        }
                    }
                }
                DfsState::Processed(handle) => {
                    // 5. Finalisation du nœud (Phase de remontée)
                    visiting.remove(&handle);

                    if !visited.contains(&handle) {
                        visited.insert(handle.clone());
                        if let Some(element) = elements_map.remove(&handle) {
                            sorted.push(element);
                        }
                    }
                }
            }
        }
    }

    Ok(sorted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_generator::models::{CodeElementType, Visibility};

    fn create_mock_element(handle: &str, deps: Vec<&str>) -> CodeElement {
        CodeElement {
            module_id: None,
            parent_id: None,
            attributes: vec![],
            docs: None,
            elements: vec![],
            handle: handle.to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            signature: format!("fn {}()", handle),
            body: None,
            dependencies: deps.into_iter().map(|s| s.to_string()).collect(),
            metadata: UnorderedMap::new(),
        }
    }

    #[test]
    fn test_successful_topological_sort() {
        let e1 = create_mock_element("app_run", vec!["db_init"]);
        let e2 = create_mock_element("db_init", vec!["config_load"]);
        let e3 = create_mock_element("config_load", vec![]);

        let sorted = sort_elements_topologically(vec![e1, e2, e3]).unwrap();

        // L'ordre doit être : config -> db -> app
        assert_eq!(sorted[0].handle, "config_load");
        assert_eq!(sorted[1].handle, "db_init");
        assert_eq!(sorted[2].handle, "app_run");
    }

    #[test]
    fn test_circular_dependency_detection() {
        let e1 = create_mock_element("A", vec!["B"]);
        let e2 = create_mock_element("B", vec!["A"]);

        let result = sort_elements_topologically(vec![e1, e2]);

        assert!(result.is_err());
        if let Err(AppError::Structured(data)) = result {
            assert_eq!(data.code, "ERR_CODEGEN_CIRCULAR_DEPENDENCY");
            // Vérification de l'observabilité RAISE
            assert!(data.context.get("handle").is_some());
        }
    }

    #[test]
    fn test_missing_dependency_resilience() {
        // Une dépendance externe (ex: fs::File) ne doit pas bloquer le tri
        let e1 = create_mock_element("write_file", vec!["fs::File"]);

        let result = sort_elements_topologically(vec![e1]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }
}
