// FICHIER : src-tauri/src/ai/agents/prompt_engine.rs

use crate::json_db::collections::manager::CollectionsManager;
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
    pub async fn compile(&self, prompt_id: &str) -> RaiseResult<String> {
        let manager = CollectionsManager::new(&self.db, &self.space, &self.db_name);

        // 1. Récupération du document JSON-LD du prompt
        let doc = match manager.get_document("prompts", prompt_id).await {
            Ok(Some(d)) => d,
            Ok(None) => {
                raise_error!(
                    "ERR_PROMPT_NOT_FOUND",
                    error = format!("Le prompt '{}' est introuvable dans la base.", prompt_id),
                    context = json_value!({ "prompt_id": prompt_id })
                )
            }
            Err(e) => raise_error!("ERR_DB_READ", error = e.to_string()),
        };

        // 2. Extraction sécurisée des champs du schéma `prompt.schema.json`
        let role = doc["role"].as_str().unwrap_or("Assistant IA");

        let persona = doc["identity"]["persona"]
            .as_str()
            .unwrap_or("Tu es une IA utile.");
        let tone = doc["identity"]["tone"].as_str().unwrap_or("professionnel");

        let environment = doc["environment"].as_str().unwrap_or("");

        let directives = doc["directives"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| format!("- {}", s))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

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

        let format_instructions = doc["format_instructions"].as_str().unwrap_or("");

        // 3. Assemblage du Prompt final (Format Markdown structuré)
        let mut compiled = format!(
            "RÔLE :\n{}\n\nPERSONA :\n{}\nTon : {}\n\nENVIRONNEMENT :\n{}\n\nDIRECTIVES :\n{}\n",
            role, persona, tone, environment, directives
        );

        if !constraints.is_empty() {
            compiled.push_str(&format!("\nCONTRAINTES (STRICT) :\n{}\n", constraints));
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
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_compile_prompt_success() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // 🎯 FIX : Utilisation du schéma générique pour contourner la validation stricte dans le bac à sable
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
                    "_id": "ref:prompts:handle:test_prompt",
                    "role": "Agent de Test",
                    "identity": { "persona": "Test Persona", "tone": "robot" },
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
            .compile("ref:prompts:handle:test_prompt")
            .await
            .unwrap();

        assert!(result.contains("RÔLE :\nAgent de Test"));
        assert!(result.contains("- Fais X"));
    }
}
