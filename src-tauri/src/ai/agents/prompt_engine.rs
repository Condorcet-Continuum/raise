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
        // On détermine où se trouve physiquement le prompt demandé.
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

        // 3. Récupération du document via Match exhaustif
        let doc = match manager.get_document(target_col, target_id).await? {
            Some(d) => d,
            None => {
                raise_error!(
                    "ERR_PROMPT_NOT_FOUND",
                    error = format!(
                        "Prompt '{}' introuvable dans la base '{}/{}'.",
                        target_id, target_space, target_db
                    ),
                    context = json_value!({ "id": target_id, "collection": target_col })
                );
            }
        };

        // 🎯 4. Validation du contrat de variables (Fail-Fast)
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

        // 🎯 5. Extraction des champs OBLIGATOIRES (Zéro Dette)
        let role = match doc["role"].as_str() {
            Some(r) => r,
            None => raise_error!(
                "ERR_PROMPT_CORRUPTION",
                error = "Propriété 'role' manquante."
            ),
        };

        let persona = match doc["identity"]["persona"].as_str() {
            Some(p) => p,
            None => raise_error!(
                "ERR_PROMPT_CORRUPTION",
                error = "Propriété 'identity.persona' manquante."
            ),
        };

        let mut environment = match doc["environment"].as_str() {
            Some(e) => e.to_string(),
            None => raise_error!(
                "ERR_PROMPT_CORRUPTION",
                error = "Propriété 'environment' manquante."
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
                error = "Propriété 'directives' manquante."
            ),
        };

        // 🎯 6. Extraction des champs OPTIONNELS (Sûr via unwrap_or)
        let tone = doc["identity"]["tone"]
            .as_str()
            .unwrap_or("professionnel et précis");

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

        // 🎯 7. Hydratation (Remplacement des {{placeholders}})
        if let Some(v_obj) = vars.and_then(|v| v.as_object()) {
            for (k, v) in v_obj {
                let placeholder = format!("{{{{{}}}}}", k);
                let val_str = if v.is_string() {
                    v.as_str().unwrap_or("").to_string()
                } else {
                    v.to_string()
                };
                environment = environment.replace(&placeholder, &val_str);
                directives = directives.replace(&placeholder, &val_str);
            }
        }

        // 🎯 8. Assemblage Final
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
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_compile_prompt_success() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await?;

        manager
            .create_collection(
                "prompts",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        manager
            .upsert_document(
                "prompts",
                json_value!({
                    "_id": "test_prompt",
                    "handle": "test_prompt",
                    "role": "Agent de Test",
                    "identity": { "persona": "Robot Testeur", "tone": "robot" },
                    "environment": "Zone de test.",
                    "directives": ["Directives 1"]
                }),
            )
            .await?;

        let engine = PromptEngine::new(
            sandbox.db.clone(),
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let result = engine
            .compile("ref:prompts:handle:test_prompt", None)
            .await?;

        assert!(result.contains("RÔLE :\nAgent de Test"));
        assert!(result.contains("Robot Testeur"));
        Ok(())
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_compile_prompt_with_absolute_smart_link() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        // 1. Configurer la base de données distante (Le domaine Système Central)
        // On simule que le prompt est stocké dans le point de montage "Raise"
        let remote_space = &config.mount_points.raise.domain;
        let remote_db = &config.mount_points.raise.db;

        let global_mgr = CollectionsManager::new(&sandbox.db, remote_space, remote_db);
        DbSandbox::mock_db(&global_mgr).await?;

        global_mgr
            .create_collection(
                "prompts",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        global_mgr
            .upsert_document(
                "prompts",
                json_value!({
                    "_id": "prompt_global",
                    "handle": "prompt_global",
                    "role": "Architecte Global",
                    "identity": { "persona": "Persona Central", "tone": "strict" },
                    "environment": "Gouvernance mondiale",
                    "directives": ["Directives Globales"]
                }),
            )
            .await?;

        // 2. Configurer le moteur sur un projet Workspace différent
        let engine = PromptEngine::new(
            sandbox.db.clone(),
            &config.mount_points.modeling.domain,
            &config.mount_points.modeling.db,
        );

        // 3. Appel de compile avec l'URI absolue vers le point de montage Raise
        let prompt_uri = format!(
            "db://{}/{}/prompts/handle/prompt_global",
            remote_space, remote_db
        );
        let result = engine.compile(&prompt_uri, None).await?;

        // 4. Vérifications
        assert!(result.contains("Architecte Global"));
        assert!(result.contains("Persona Central"));
        Ok(())
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_err_prompt_missing_variable() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await?;

        manager
            .create_collection(
                "prompts",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;
        manager
            .insert_raw(
                "prompts",
                &json_value!({
                    "_id": "p_var",
                    "input_variables": ["user_name"],
                    "role": "Test",
                    "identity": {"persona": "X"},
                    "environment": "Env",
                    "directives": []
                }),
            )
            .await?;

        let engine = PromptEngine::new(
            sandbox.db.clone(),
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        let result = engine.compile("p_var", None).await;

        match result {
            Err(e) if e.to_string().contains("ERR_PROMPT_MISSING_VARIABLE") => Ok(()),
            _ => panic!("Le moteur aurait dû détecter la variable manquante"),
        }
    }
}
