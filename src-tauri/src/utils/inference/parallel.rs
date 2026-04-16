// FICHIER : src-tauri/src/utils/inference/parallel.rs

use crate::utils::prelude::*;

// ⚡ PARALLÉLISME CPU (Forteresse Rayon)
//
// Cette encapsulation masque totalement la librairie de multi-threading `rayon`.
// Elle permet d'exécuter des opérations intensives (comme le mapping
// de gros volumes de données ou le pré-traitement SysML) sur tous
// les cœurs du CPU sans exposer la mécanique sous-jacente au reste de l'app.

/// Exécute une fonction de transformation en parallèle sur un ensemble de données.
/// Les contraintes `Send` et `Sync` garantissent la sécurité absolue de la mémoire (Thread-Safety) de Rust.
pub fn execute_parallel_map<T, R, F>(items: Vec<T>, op: F) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(T) -> R + Sync + Send,
{
    // L'import de rayon est confiné au scope de cette fonction.
    use rayon::prelude::*;

    // La magie du multi-threading s'opère ici, et uniquement ici.
    items.into_par_iter().map(op).collect()
}

/// Configure le pool de threads global alloué à l'inférence et aux calculs lourds.
/// À appeler une seule fois lors du démarrage (Bootstrap) de l'application.
pub fn configure_parallel_pool(threads: usize) -> RaiseResult<()> {
    match rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
    {
        Ok(_) => Ok(()),
        Err(e) => {
            // Fail-Fast structuré : on informe l'orchestrateur si le pool
            // n'a pas pu être instancié (ex: pool déjà initialisé).
            raise_error!(
                "ERR_INFERENCE_THREADPOOL_INIT",
                error = e,
                context = json_value!({
                    "requested_threads": threads,
                    "action": "init_rayon_pool",
                    "hint": "Le pool de threads a peut-être déjà été initialisé."
                })
            );
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_parallel_map_basic() {
        let input = vec![1, 2, 3, 4, 5];
        // Transformation simple pour vérifier que le routage Rayon fonctionne
        let result = execute_parallel_map(input, |x| x * 2);
        assert_eq!(result, vec![2, 4, 6, 8, 10]);
    }

    #[test]
    fn test_execute_parallel_map_large_dataset() {
        // Test de robustesse sur 10 000 éléments
        let input: Vec<u64> = (1..=10_000).collect();
        let result = execute_parallel_map(input, |x| x.pow(2));

        assert_eq!(
            result.len(),
            10_000,
            "Le nombre d'éléments ne correspond pas"
        );
        assert_eq!(result[0], 1, "Le premier élément est incorrect");
        assert_eq!(
            result[9_999], 100_000_000,
            "Le dernier élément est incorrect"
        );
    }

    #[test]
    fn test_configure_parallel_pool_safety() {
        // NOTE : Rayon ne permet de configurer le pool global qu'UNE SEULE FOIS.
        // Comme les tests tournent en parallèle, il est fortement probable que le pool
        // soit déjà initialisé. On vérifie ici que notre encapsulation RaiseResult
        // capture bien cette erreur métier sans faire paniquer le thread.
        let result = configure_parallel_pool(2);

        match result {
            Ok(_) => {
                // Si par miracle on est le premier test à s'exécuter, c'est un succès.
                assert!(true);
            }
            Err(e) => {
                // Si le pool existe déjà, notre macro raise_error! doit avoir levé ce code précis :
                assert!(
                    e.to_string().contains("ERR_INFERENCE_THREADPOOL_INIT"),
                    "L'erreur remontée n'a pas la bonne structure sémantique"
                );
            }
        }
    }
}
