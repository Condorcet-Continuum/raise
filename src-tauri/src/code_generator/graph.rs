use crate::code_generator::models::CodeElement;
use crate::utils::prelude::*;

/// 🧠 Ordonne les éléments de code pour garantir une compilation sans erreur.
pub fn sort_elements_topologically(elements: Vec<CodeElement>) -> RaiseResult<Vec<CodeElement>> {
    let mut sorted = Vec::new();
    let mut visited = UniqueSet::new(); // Éléments totalement traités
    let mut visiting = UniqueSet::new(); // Éléments en cours (pour détection de cycle)

    // Indexation par handle pour un accès rapide (O(1)) via la façade RAISE
    let mut elements_map: UnorderedMap<String, CodeElement> = elements
        .into_iter()
        .map(|e| (e.handle.clone(), e))
        .collect();

    let handles: Vec<String> = elements_map.keys().cloned().collect();

    for handle in handles {
        dfs_visit(
            &handle,
            &mut elements_map,
            &mut visited,
            &mut visiting,
            &mut sorted,
        )?;
    }

    Ok(sorted)
}

fn dfs_visit(
    handle: &str,
    elements: &mut UnorderedMap<String, CodeElement>,
    visited: &mut UniqueSet<String>,
    visiting: &mut UniqueSet<String>,
    sorted: &mut Vec<CodeElement>,
) -> RaiseResult<()> {
    // 1. Détection de cycle (Ligne rouge sémantique)
    if visiting.contains(handle) {
        raise_error!(
            "ERR_CODEGEN_CIRCULAR_DEPENDENCY",
            error = format!("Cycle détecté impliquant l'élément : {}", handle),
            context = json_value!({ "handle": handle })
        );
    }

    // 2. Si déjà traité, on ignore
    if visited.contains(handle) {
        return Ok(());
    }

    // 3. Marquage "en cours"
    visiting.insert(handle.to_string());

    // 4. Exploration des dépendances
    if let Some(element) = elements.get(handle) {
        let deps = element.dependencies.clone();
        for dep_handle in deps {
            // On ne visite la dépendance que si elle existe dans le scope actuel
            if elements.contains_key(&dep_handle) {
                dfs_visit(&dep_handle, elements, visited, visiting, sorted)?;
            }
        }
    }

    // 5. Finalisation du nœud
    visiting.remove(handle);
    visited.insert(handle.to_string());

    if let Some(element) = elements.remove(handle) {
        sorted.push(element);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_generator::models::{CodeElementType, Visibility};

    fn create_mock_element(handle: &str, deps: Vec<&str>) -> CodeElement {
        CodeElement {
            // 🎯 NOUVEAUX CHAMPS (Initialisation par défaut pour le mock)
            module_id: None,
            parent_id: None,
            attributes: vec![],
            docs: None,
            elements: vec![],

            // Champs existants
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
