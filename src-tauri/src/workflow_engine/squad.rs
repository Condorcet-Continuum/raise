// FICHIER : src-tauri/src/workflow_engine/squad.rs
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct Squad {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub _id: Option<String>,
    pub handle: String,
    pub name: I18nString,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<I18nString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<UniqueId>,
    pub lead_agent_id: UniqueId,
    #[serde(default)]
    pub agents: Vec<UniqueId>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub status: SquadStatus,
}

#[derive(Debug, Clone, Copy, Serializable, Deserializable, PartialEq, Eq)]
#[serde(rename_all = "lowercase")] // active, training, suspended, retired
pub enum SquadStatus {
    Active,
    Training,
    Suspended,
    Retired,
}

impl Squad {
    /// Récupère une Escouade IA depuis le Knowledge Graph
    pub async fn fetch_from_store(
        manager: &CollectionsManager<'_>,
        handle: &str,
    ) -> RaiseResult<Self> {
        let doc_result = manager.get_document("squads", handle).await;
        let doc = match doc_result {
            Ok(Some(d)) => d,
            Ok(None) => raise_error!(
                "ERR_WF_SQUAD_NOT_FOUND",
                context = json_value!({"squad_handle": handle})
            ),
            Err(e) => raise_error!(
                "ERR_WF_SQUAD_DB_ACCESS",
                error = e.to_string(),
                context = json_value!({"squad_handle": handle})
            ),
        };

        let mut squad: Squad = match json::deserialize_from_value(doc) {
            Ok(s) => s,
            Err(e) => raise_error!(
                "ERR_WF_SQUAD_CORRUPT",
                error = e.to_string(),
                context = json_value!({"squad_handle": handle})
            ),
        };

        squad._id = Some(handle.to_string());
        Ok(squad)
    }
}

// ============================================================================
// TESTS UNITAIRES ROBUSTES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::test_utils::init_test_env;
    use crate::utils::testing::DbSandbox;

    #[async_test]
    async fn test_fetch_squad_success() {
        let env = init_test_env().await;
        let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
        DbSandbox::mock_db(&manager).await.unwrap();

        manager
            .create_collection(
                "squads",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        // 🎯 FIX : Utilisation d'UUID valides pour respecter le type UniqueId
        let lead_id = "10000000-0000-0000-0000-000000000001";
        let agent_id = "10000000-0000-0000-0000-000000000002";

        let squad_json = json_value!({
            "handle": "squad-alpha",
            "name": "Alpha Squad",
            "description": "Elite AI unit",
            "lead_agent_id": lead_id,
            "agents": [agent_id],
            "capabilities": ["code_generation", "review"],
            "status": "active"
        });

        manager.upsert_document("squads", squad_json).await.unwrap();

        let result = Squad::fetch_from_store(&manager, "squad-alpha").await;
        assert!(
            result.is_ok(),
            "La Squad devrait être trouvée et parsée avec succès"
        );

        let squad = result.unwrap();
        assert_eq!(squad.handle, "squad-alpha");
        assert_eq!(squad.status, SquadStatus::Active);
        assert_eq!(squad.lead_agent_id.to_string(), lead_id);
        assert_eq!(squad.agents.len(), 1);
        assert_eq!(squad.capabilities.len(), 2);
    }

    #[async_test]
    async fn test_fetch_squad_not_found() {
        let env = init_test_env().await;
        let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
        DbSandbox::mock_db(&manager).await.unwrap();

        manager
            .create_collection(
                "squads",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        // Tentative de récupération d'une Squad inexistante
        let result = Squad::fetch_from_store(&manager, "unknown-squad").await;

        assert!(result.is_err(), "Devrait retourner une erreur");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("ERR_WF_SQUAD_NOT_FOUND"));
    }

    #[async_test]
    async fn test_fetch_squad_corrupt() {
        let env = init_test_env().await;
        let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
        DbSandbox::mock_db(&manager).await.unwrap();

        manager
            .create_collection(
                "squads",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        // 🎯 On omet volontairement le champ requis `lead_agent_id`
        let corrupt_squad_json = json_value!({
            "handle": "squad-broken",
            "name": "Broken Squad",
            "status": "active"
        });

        manager
            .upsert_document("squads", corrupt_squad_json)
            .await
            .unwrap();

        // Tentative de récupération
        let result = Squad::fetch_from_store(&manager, "squad-broken").await;

        assert!(
            result.is_err(),
            "Devrait échouer car la donnée est corrompue/incomplète"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("ERR_WF_SQUAD_CORRUPT"));
    }
}
