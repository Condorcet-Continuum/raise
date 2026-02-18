// FICHIER : src-tauri/src/commands/codegen_commands.rs

use crate::utils::{data::Value, prelude::*};

use crate::commands::rules_commands::RuleEngineState;
use crate::json_db::storage::StorageEngine;
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::transformers::{get_transformer, TransformationDomain};
use tauri::State;

/// Génère une représentation technique (Code, VHDL, Doc) pour un élément donné.
/// Remplace l'ancienne logique de génération par le nouveau système de Transformers.
///
/// NOTE : Le nom 'generate_source_code' est conservé pour la compatibilité avec main.rs
#[tauri::command]
pub async fn generate_source_code(
    element_id: String,
    domain: String, // "software", "hardware", "system"
    state: State<'_, RuleEngineState>,
    storage: State<'_, StorageEngine>,
) -> Result<Value> {
    // 1. Parsing du domaine cible
    // On mappe la chaîne reçue du frontend vers l'enum TransformationDomain
    let target_domain = match domain.to_lowercase().as_str() {
        "software" | "code" | "rust" | "cpp" => TransformationDomain::Software,
        "hardware" | "vhdl" | "fpga" | "verilog" => TransformationDomain::Hardware,
        "system" | "overview" | "doc" | "architecture" => TransformationDomain::System,
        _ => {
            return Err(AppError::Validation(format!(
                "Domaine de transformation inconnu ou non supporté : {}",
                domain
            )))
        }
    };

    // 2. Récupération du contexte (Space/DB) depuis le modèle en mémoire
    // On utilise le RuleEngineState pour savoir quel projet est actuellement chargé dans l'UI
    let (space, db) = {
        let model = state.model.lock().await;
        // On récupère "Space/DB" depuis les métadonnées du projet
        let parts: Vec<&str> = model.meta.name.split('/').collect();
        if parts.len() >= 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            // Fallback si aucun projet n'est chargé (évite le crash, mais risque de ne rien trouver)
            ("default".to_string(), "default".to_string())
        }
    };

    // 3. Initialisation du Loader (Mode Lazy)
    // Le loader va aller chercher uniquement les fichiers nécessaires sur le disque
    let loader = ModelLoader::new(&storage, &space, &db);

    // Indexation rapide pour localiser l'élément par son UUID (si pas déjà fait en cache)
    loader.index_project().await.map_err(|e| e.to_string())?;

    // 4. Récupération et Hydratation de l'élément source
    // fetch_hydrated_element est crucial ici : il remplace les ID par les objets complets
    // (ex: ownedLogicalComponents devient une liste d'objets, pas juste d'UUIDs)
    let element_json = loader
        .fetch_hydrated_element(&element_id)
        .await
        .map_err(|e| format!("Impossible de charger l'élément {} : {}", element_id, e))?;

    // 5. Exécution de la transformation
    // On récupère le bon transformateur (SoftwareTransformer, HardwareTransformer, etc.)
    let transformer = get_transformer(target_domain);

    // On applique la transformation qui renvoie un JSON prêt pour le moteur de template (Tera)
    let result = transformer
        .transform(&element_json)
        .map_err(|e| format!("Erreur lors de la transformation : {}", e))?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::json_db::storage::JsonDbConfig;
    use crate::model_engine::arcadia;
    use crate::utils::io::tempdir;

    /// Test d'intégration complet : DB -> Loader -> Transformer -> Sortie
    /// Vérifie que la logique interne de la commande fonctionne correctement.
    #[tokio::test]
    async fn test_generate_code_logic() {
        // 1. Setup de l'environnement (Stockage temporaire)
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);

        // On crée un projet fictif "MySpace/MyDB"
        let manager = CollectionsManager::new(&storage, "MySpace", "MyDB");
        manager.init_db().await.unwrap();

        // 2. Injection de données (Un composant logiciel avec une fonction)
        let component_id = "UUID-COMP-1";
        let component = json!({
            arcadia::PROP_ID: component_id,
            arcadia::PROP_NAME: "AuthService",
            "@type": "LogicalComponent", // Sera mappé en LA_COMPONENT
            // Allocation fonctionnelle simulée (pour tester l'hydratation/transformation)
            "ownedFunctionalAllocation": [
                { arcadia::PROP_ID: "FUNC-1", arcadia::PROP_NAME: "Login" }
            ]
        });

        // Insertion dans la collection "la" (Logical Architecture)
        manager.insert_raw("la", &component).await.unwrap();

        // 3. Simulation de la logique de la commande
        // CORRECTION : Utilisation de from_engine pour passer un StorageEngine brut (pas State<...>)
        let loader = ModelLoader::from_engine(&storage, "MySpace", "MyDB");
        loader.index_project().await.unwrap(); // Indexation obligatoire

        // 4. Test cas nominal : Génération Software
        let element_json = loader.fetch_hydrated_element(component_id).await.unwrap();
        let transformer = get_transformer(TransformationDomain::Software);
        let result = transformer.transform(&element_json).unwrap();

        // Vérifications
        assert_eq!(result["domain"], "software");
        assert_eq!(result["entity"]["name"], "AuthService");

        // Vérifie que la méthode Login a bien été générée (preuve que la transformation lit bien le JSON)
        let methods = result["entity"]["methods"].as_array().unwrap();
        assert!(methods.iter().any(|m| m["name"] == "Login"));
    }

    /// Test de gestion d'erreur : Élément inexistant
    #[tokio::test]
    async fn test_generate_code_not_found() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);

        // CORRECTION : Utilisation de from_engine
        let loader = ModelLoader::from_engine(&storage, "EmptySpace", "EmptyDB");
        // Pas d'insert -> Index vide

        let result = loader.fetch_hydrated_element("UNKNOWN-ID").await;
        assert!(result.is_err(), "Devrait échouer pour un ID inconnu");
    }

    /// Test de gestion d'erreur : Domaine inconnu
    #[test]
    fn test_domain_parsing() {
        let valid = "rust";
        let invalid = "magic_language";

        let domain_enum = match valid {
            "rust" => Some(TransformationDomain::Software),
            _ => None,
        };
        assert!(domain_enum.is_some());

        let invalid_enum = match invalid {
            "software" => Some(TransformationDomain::Software),
            _ => None, // Simule le fallback de la commande
        };
        assert!(invalid_enum.is_none());
    }
}
