// FICHIER : src-tauri/src/ai/agents/prompt_engine.rs

use crate::json_db::collections::manager::{parse_smart_link, CollectionsManager, SmartLink};
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*;

/// Le `PromptEngine` est responsable de la compilation dynamique des contextes IA.
/// Il lit les définitions formelles (JSON-LD) en DB et les transforme en instructions textuelles
/// compréhensibles par les modèles de fondation (LLM).
pub struct PromptEngine {
    db: SharedRef<StorageEngine>,
    space: String,
    db_name: String,
}

impl PromptEngine {
    pub fn new(db: SharedRef<StorageEngine>, space: &str, db_name: &str) -> Self {
        Self {
            db,
            space: space.to_string(),
            db_name: db_name.to_string(),
        }
    }

    /// Compile un prompt complet à partir de son ID ou de sa référence (URN) en DB.
    pub async fn compile(&self, prompt_id: &str, vars: Option<&JsonValue>) -> RaiseResult<String> {
        // 🎯 1. RÉSOLUTION DE L'URI (Smart Link)
        let (target_space, target_db, target_col, target_id) = match parse_smart_link(prompt_id) {
            Some(SmartLink::Absolute {
                space,
                db,
                col,
                val,
                ..
            }) => (space, db, col, val),
            Some(SmartLink::Local { col, val, .. }) => {
                (self.space.as_str(), self.db_name.as_str(), col, val)
            }
            None => (
                self.space.as_str(),
                self.db_name.as_str(),
                "prompts",
                prompt_id,
            ), // Fallback local
        };

        // 🎯 2. INITIALISATION DYNAMIQUE DU MANAGER
        let manager = CollectionsManager::new(&self.db, target_space, target_db);

        // 3. Récupération du document sur la bonne base et la bonne collection
        let doc = match manager.get_document(target_col, target_id).await {
            Ok(Some(d)) => d,
            Ok(None) => raise_error!(
                "ERR_PROMPT_NOT_FOUND",
                error = format!(
                    "Prompt '{}' introuvable dans la base '{}/{}'.",
                    target_id, target_space, target_db
                )
            ),
            Err(e) => raise_error!("ERR_DB_READ", error = e.to_string()),
        };

        // 🎯 4 Validation du contrat de variables (Fail-Fast)
        if let Some(expected_vars) = doc["input_variables"].as_array() {
            let provided_vars = vars.and_then(|v| v.as_object());
            for var_name in expected_vars.iter().filter_map(|v| v.as_str()) {
                if provided_vars.is_none_or(|obj| !obj.contains_key(var_name)) {
                    raise_error!(
                        "ERR_PROMPT_MISSING_VARIABLE",
                        error = format!(
                            "Variable '{}' manquante pour le prompt '{}'.",
                            var_name, prompt_id
                        )
                    );
                }
            }
        }

        // 🎯 5. Extraction des champs OBLIGATOIRES (Match + raise_error!)
        let role = match doc["role"].as_str() {
            Some(r) => r,
            None => raise_error!("ERR_PROMPT_CORRUPTION", error = "Champ 'role' manquant."),
        };

        let persona = match doc["identity"]["persona"].as_str() {
            Some(p) => p,
            None => raise_error!(
                "ERR_PROMPT_CORRUPTION",
                error = "Champ 'identity.persona' manquant."
            ),
        };

        let mut environment = match doc["environment"].as_str() {
            Some(e) => e.to_string(),
            None => raise_error!(
                "ERR_PROMPT_CORRUPTION",
                error = "Champ 'environment' manquant."
            ),
        };

        let mut directives = match doc["directives"].as_array() {
            Some(arr) => arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!("- {}", s))
                .collect::<Vec<_>>()
                .join("\n"),
            None => raise_error!(
                "ERR_PROMPT_CORRUPTION",
                error = "Champ 'directives' manquant."
            ),
        };

        // 🎯 6. Extraction des champs OPTIONNELS (Idiomatic Rust)
        let tone = doc["identity"]["tone"]
            .as_str()
            .unwrap_or("professionnel, précis et déterministe");

        let constraints = doc["constraints"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| format!("- {}", s))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        let format_instructions = doc["format_instructions"].as_str().unwrap_or_default();

        // 🎯 7. Hydratation (IF LET : Satisfait Clippy single-match)
        if let Some(v_obj) = vars.and_then(|v| v.as_object()) {
            for (k, v) in v_obj {
                let placeholder = format!("{{{{{}}}}}", k);
                let val_str = if v.is_string() {
                    v.as_str().unwrap().to_string()
                } else {
                    v.to_string()
                };
                environment = environment.replace(&placeholder, &val_str);
                directives = directives.replace(&placeholder, &val_str);
            }
        }

        // 🎯 8. Assemblage
        let mut compiled = format!(
            "RÔLE :\n{}\n\nPERSONA :\n{}\nTon : {}\n\nENVIRONNEMENT :\n{}\n\nDIRECTIVES :\n{}\n",
            role, persona, tone, environment, directives
        );

        if !constraints.is_empty() {
            compiled.push_str(&format!("\nCONTRAINTES :\n{}\n", constraints));
        }

        if !format_instructions.is_empty() {
            compiled.push_str(&format!("\nFORMAT DE SORTIE :\n{}\n", format_instructions));
        }

        Ok(compiled)
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{AgentDbSandbox, DbSandbox};

    #[async_test]
    async fn test_compile_prompt_success() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        DbSandbox::mock_db(&manager).await.unwrap();

        // Utilisation du schéma générique pour contourner la validation stricte dans le bac à sable
        manager
            .create_collection(
                "prompts",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        manager
            .upsert_document(
                "prompts",
                json_value!({
                    "_id": "test_prompt",
                    "handle": "test_prompt",
                    "name": { "fr": "Prompt de Test" },
                    "role": "Agent de Test",
                    "identity": { "persona": "Test Persona", "tone": "robot" },
                    "environment": "Tu opères dans un environnement de test unitaire.",
                    "directives": ["Fais X", "Fais Y"]
                }),
            )
            .await
            .unwrap();

        let engine = PromptEngine::new(
            sandbox.db.clone(),
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        let result = engine
            .compile("ref:prompts:handle:test_prompt", None)
            .await
            .unwrap();

        assert!(result.contains("RÔLE :\nAgent de Test"));
        assert!(result.contains("- Fais X"));
    }

    #[async_test]
    async fn test_compile_prompt_with_absolute_smart_link() {
        let sandbox = AgentDbSandbox::new().await;
        // 1. Configurer la base de données distante (Le domaine Système Central)
        let global_mgr = CollectionsManager::new(&sandbox.db, "_system", "raise");
        DbSandbox::mock_db(&global_mgr).await.unwrap();

        // 1. Configurer la base de données distante (Le domaine Système Central)
        let global_mgr = CollectionsManager::new(&sandbox.db, "_system", "raise");
        global_mgr
            .create_collection(
                "prompts",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        global_mgr
            .upsert_document(
                "prompts",
                json_value!({
                    "_id": "prompt_global_mandate",
                    "handle": "prompt_global_mandate",
                    "role": "Agent Global Architect",
                    "identity": { "persona": "Je suis le système global", "tone": "strict" },
                    "environment": "Gouvernance globale",
                    "directives": ["Appliquer la méthode Arcadia"]
                }),
            )
            .await
            .unwrap();

        // 2. Configurer le moteur de prompt sur un projet LOCAL différent
        // Il est instancié sur 'mbse2/raise', il ne possède théoriquement pas le prompt global en local.
        let engine = PromptEngine::new(sandbox.db.clone(), "mbse2", "raise");

        // 3. Appel de compile avec l'URI absolue (db://espace/base/collection/champ/valeur)
        let prompt_uri = "db://_system/raise/prompts/handle/prompt_global_mandate";
        let result = engine.compile(prompt_uri, None).await.unwrap();

        // 4. Vérifications (Preuve que le PromptEngine a voyagé jusqu'au bon domaine)
        assert!(
            result.contains("RÔLE :\nAgent Global Architect"),
            "Le rôle global devrait être trouvé."
        );
        assert!(
            result.contains("Je suis le système global"),
            "Le persona global devrait être récupéré."
        );
        assert!(
            result.contains("- Appliquer la méthode Arcadia"),
            "La directive du domaine externe devrait être incluse."
        );
    }
}
