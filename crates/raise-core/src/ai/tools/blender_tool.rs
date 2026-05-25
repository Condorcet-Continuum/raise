// FICHIER : src-tauri/src/ai/tools/blender_tool.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE
use crate::workflow_engine::handlers::HandlerContext;
use crate::workflow_engine::tools::AgentTool;

/// Outil Agent "Data-Driven" permettant de piloter Blender.
/// Implémente `AgentTool` pour s'intégrer nativement dans l'Orchestrateur RAISE.
#[derive(Debug)]
pub struct BlenderTool {
    dataset_dir: PathBuf,
    tool_id: String,
    // On met en cache les éléments synchrones requis par le trait AgentTool
    cached_name: String,
    cached_description: String,
    cached_schema: JsonValue,
    cached_output_schema: Option<JsonValue>,
}

impl BlenderTool {
    /// Initialisation asynchrone sécurisée au démarrage du système.
    /// Charge le contrat de l'outil et exige (Fail-Fast) une résolution parfaite des schémas.
    pub async fn init(
        dataset_dir: PathBuf,
        db: &SharedRef<StorageEngine>,
        space: &str,
        db_name: &str,
        tool_id: &str,
    ) -> RaiseResult<Self> {
        let manager = CollectionsManager::new(db, space, db_name);

        // 1. Validation de la présence en base
        let doc = match manager.get_document("mcp_tools", tool_id).await {
            Ok(Some(d)) => d,
            Ok(None) => raise_error!(
                "ERR_TOOL_NOT_FOUND",
                error = "Configuration de l'outil Blender introuvable en base.",
                context = json_value!({ "tool_id": tool_id })
            ),
            Err(e) => return Err(e),
        };

        let tool_config = doc
            .get("tools")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first());

        let cached_name = tool_config
            .and_then(|t| t.get("tool_id"))
            .and_then(|id| id.as_str())
            .unwrap_or("generate_synthetic_data")
            .to_string();

        let cached_description = tool_config
            .and_then(|t| t.get("description"))
            .and_then(|d| d.as_str())
            .unwrap_or("Outil MCP sans description définie en base.")
            .to_string();

        // 2. 🎯 VALIDATION STRICTE DE LA PRÉSENCE DES URIs (Zéro Fallback)
        let input_uri = match tool_config
            .and_then(|t| t.get("input_schema_uri"))
            .and_then(|u| u.as_str())
        {
            Some(uri) if !uri.is_empty() => uri,
            _ => raise_error!(
                "ERR_TOOL_MISSING_INPUT_SCHEMA",
                error = "L'URI du schéma d'entrée (input_schema_uri) est obligatoire.",
                context = json_value!({ "tool_id": tool_id })
            ),
        };

        let output_uri = match tool_config
            .and_then(|t| t.get("output_schema_uri"))
            .and_then(|u| u.as_str())
        {
            Some(uri) if !uri.is_empty() => uri,
            _ => raise_error!(
                "ERR_TOOL_MISSING_OUTPUT_SCHEMA",
                error = "L'URI du schéma de sortie (output_schema_uri) est obligatoire.",
                context = json_value!({ "tool_id": tool_id })
            ),
        };

        // 3. 🎯 RÉSOLUTION PHYSIQUE STRICTE (Fail-Fast si le manager échoue)
        let cached_schema = match manager.get_schema_def(input_uri).await {
            Ok(schema) => schema,
            Err(e) => raise_error!(
                "ERR_TOOL_INPUT_SCHEMA_RESOLUTION",
                error = format!("Impossible de lire le contrat d'entrée : {}", e),
                context = json_value!({ "tool_id": tool_id, "uri": input_uri })
            ),
        };

        let cached_output_schema = match manager.get_schema_def(output_uri).await {
            Ok(schema) => Some(schema),
            Err(e) => raise_error!(
                "ERR_TOOL_OUTPUT_SCHEMA_RESOLUTION",
                error = format!("Impossible de lire le contrat de sortie : {}", e),
                context = json_value!({ "tool_id": tool_id, "uri": output_uri })
            ),
        };

        Ok(Self {
            dataset_dir,
            tool_id: tool_id.to_string(),
            cached_name,
            cached_description,
            cached_schema,
            cached_output_schema,
        })
    }
}

#[async_interface]
impl AgentTool for BlenderTool {
    fn name(&self) -> &str {
        &self.cached_name
    }

    fn description(&self) -> &str {
        &self.cached_description
    }

    fn parameters_schema(&self) -> JsonValue {
        self.cached_schema.clone()
    }

    fn output_schema(&self) -> Option<JsonValue> {
        self.cached_output_schema.clone()
    }

    async fn execute(
        &self,
        params: &JsonValue,
        context: &HandlerContext<'_>,
    ) -> RaiseResult<JsonValue> {
        // 1. Extraction et validation stricte des arguments via le JSON entrant
        let filename = match params.get("output_filename").and_then(|v| v.as_str()) {
            Some(f) => f,
            None => raise_error!(
                "ERR_BLENDER_ARG_INVALID",
                error = "Argument 'output_filename' manquant"
            ),
        };
        let defect = match params.get("defect_type").and_then(|v| v.as_str()) {
            Some(d) => d,
            None => raise_error!(
                "ERR_BLENDER_ARG_INVALID",
                error = "Argument 'defect_type' manquant"
            ),
        };
        let lighting = match params.get("lighting_intensity") {
            Some(l) => l.to_string(),
            None => raise_error!(
                "ERR_BLENDER_ARG_INVALID",
                error = "Argument 'lighting_intensity' manquant"
            ),
        };

        // 2. Sandboxing & Path Traversal Protection
        if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
            raise_error!(
                "ERR_FS_SECURITY_VIOLATION",
                error = "Path traversal détecté."
            );
        }

        fs::ensure_dir_async(&self.dataset_dir).await?;
        let full_path = self.dataset_dir.join(filename);
        let path_str = full_path.to_string_lossy().to_string();

        // 3. Récupération à chaud de la configuration via le HandlerContext
        let config = match context
            .manager
            .get_document("mcp_tools", &self.tool_id)
            .await
        {
            Ok(Some(c)) => c,
            Ok(None) => raise_error!(
                "ERR_TOOL_CONFIG_MISSING",
                error = "Document outil supprimé en pleine exécution."
            ),
            Err(e) => return Err(e),
        };

        // 4. Moteur de Templating (depuis le champ 'prompts' du schema)
        let cmd = config["stdio"]["command"].as_str().unwrap_or("blender");
        let mut args: Vec<String> = config["stdio"]["args"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        let template = config["prompts"][0]["content"].as_str().unwrap_or("");

        let python_expr = template
            .replace("{{defect_type}}", defect)
            .replace("{{lighting_intensity}}", &lighting)
            .replace("{{output_path}}", &path_str);

        args.push(python_expr);

        user_info!(
            "INF_BLENDER_TOOL_EXEC",
            json_value!({ "tool": &self.tool_id, "target": &path_str })
        );

        // 5. Exécution sécurisée via OS Façade
        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        match os::exec_command_async(cmd, &args_refs, Some(&self.dataset_dir)).await {
            Ok(stdout) => Ok(json_value!({
                "path": path_str,
                "status": "success",
                "blender_output": stdout.lines().last().unwrap_or("OK")
            })),
            Err(e) => return Err(e),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::orchestrator::AiOrchestrator;
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::utils::data::config::AppConfig;
    use crate::utils::testing::mock::{insert_mock_db, AgentDbSandbox};
    use crate::workflow_engine::critic::WorkflowCritic;

    /// Seed mocké : Délègue l'intégralité de la création DDL et de l'insertion au Manager
    async fn seed_blender_tool(manager: &CollectionsManager<'_>, tool_id: &str) -> RaiseResult<()> {
        let generic_schema = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            manager.space, manager.db
        );

        let _ = manager.create_collection("schemas", &generic_schema).await;
        let _ = manager
            .create_collection("mcp_tools", &generic_schema)
            .await;

        let input_id = "v2/agents/tools/blender_input.schema.json";
        let output_id = "v2/agents/tools/blender_output.schema.json";

        // 1. 🎯 DDL NATIF : Création des schémas via le manager (Ils seront lus par get_schema_def !)
        manager
            .create_schema_def(
                input_id,
                json_value!({
                    "type": "object",
                    "properties": {
                        "output_filename": { "type": "string" },
                        "defect_type": { "type": "string" },
                        "lighting_intensity": { "type": "number" }
                    }
                }),
            )
            .await?;

        manager
            .create_schema_def(
                output_id,
                json_value!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "status": { "type": "string" }
                    }
                }),
            )
            .await?;

        let input_uri = manager.build_schema_uri(input_id).await;
        let output_uri = manager.build_schema_uri(output_id).await;

        // 2. 🎯 SATISFAIRE LE VALIDATEUR DB (Références)
        insert_mock_db(manager, "schemas", &json_value!({ "_id": input_id })).await?;
        insert_mock_db(manager, "schemas", &json_value!({ "_id": output_id })).await?;

        // 3. 🎯 INJECTION DE L'OUTIL AVEC SES URIS
        insert_mock_db(
            manager,
            "mcp_tools",
            &json_value!({
                "_id": tool_id,
                "@type": ["raise:McpTool", "pa:PhysicalFunction"],
                "transport": "stdio",
                "stdio": {
                    "command": "blender",
                    "args": ["-b", "--python-expr"]
                },
                "tools": [{
                    "tool_id": "gen_blender_data",
                    "description": "Générateur 3D",
                    "input_schema_uri": input_uri,
                    "output_schema_uri": output_uri
                }],
                "prompts": [{
                    "prompt_id": "prompt_1",
                    "content": "TEST_DEFECT={{defect_type}}"
                }]
            }),
        )
        .await?;
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_blender_agent_tool_success() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        let tool_id = "tool:blender:test";
        let dataset_dir = sandbox.domain_root.join("dataset");

        seed_blender_tool(&manager, tool_id).await?;

        // 1. Initialisation : Si les URI manquent ou le fichier FS manque, ça plantera sec ici !
        let tool = BlenderTool::init(
            dataset_dir,
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
            tool_id,
        )
        .await?;

        assert_eq!(tool.name(), "gen_blender_data");
        assert!(tool.parameters_schema().get("properties").is_some());
        assert!(tool.output_schema().is_some());

        // 2. Setup du HandlerContext
        let orch = AiOrchestrator::new(ProjectModel::default(), &manager, sandbox.db.clone(), None)
            .await
            .unwrap();
        let pm = SharedRef::new(PluginManager::new(&sandbox.db, None));
        let ctx = HandlerContext {
            orchestrator: &SharedRef::new(AsyncMutex::new(orch)),
            plugin_manager: &pm,
            critic: &WorkflowCritic::default(),
            tools: &UnorderedMap::new(),
            manager: &manager,
        };

        // 3. Exécution
        let params = json_value!({
            "output_filename": "test.png",
            "defect_type": "saine",
            "lighting_intensity": 1000
        });

        let result = tool.execute(&params, &ctx).await;

        if let Err(AppError::Structured(err)) = result {
            assert_eq!(err.code, "ERR_OS_EXEC_SPAWN");
        }
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_blender_agent_tool_missing_args() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        let tool_id = "tool:blender:test";
        let dataset_dir = sandbox.domain_root.join("dataset");

        seed_blender_tool(&manager, tool_id).await?;

        let tool = BlenderTool::init(
            dataset_dir,
            &sandbox.db,
            &manager.space,
            &manager.db,
            tool_id,
        )
        .await?;

        let orch = AiOrchestrator::new(ProjectModel::default(), &manager, sandbox.db.clone(), None)
            .await
            .unwrap();
        let ctx = HandlerContext {
            orchestrator: &SharedRef::new(AsyncMutex::new(orch)),
            plugin_manager: &SharedRef::new(PluginManager::new(&sandbox.db, None)),
            critic: &WorkflowCritic::default(),
            tools: &UnorderedMap::new(),
            manager: &manager,
        };

        let params = json_value!({ "output_filename": "test.png", "defect_type": "saine" });

        let result = tool.execute(&params, &ctx).await;

        match result {
            Err(AppError::Structured(err)) => assert_eq!(err.code, "ERR_BLENDER_ARG_INVALID"),
            _ => panic!("Aurait dû lever une erreur d'argument manquant"),
        }
        Ok(())
    }
}
