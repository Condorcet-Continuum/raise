// FICHIER : src-tauri/src/workflow_engine/rbac.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::rules_engine::ast::Expr;
use crate::rules_engine::evaluator::{Evaluator, NoOpDataProvider};
use crate::utils::prelude::*;

use super::mandate::{ActionType, Mandator, Permission, Role};

pub struct RbacEngine;

impl RbacEngine {
    /// Vérifie si un Mandant possède une permission spécifique.
    /// Utilisation stricte de match ... raise_error! (sans return redondant).
    pub async fn verify_access(
        manager: &CollectionsManager<'_>,
        mandator_id: &UniqueId,
        target_service: &str,
        target_action: ActionType,
        evaluation_context: &JsonValue,
    ) -> RaiseResult<()> {
        user_info!(
            "RBAC_VERIFY_START",
            json_value!({"mandator_id": mandator_id, "service": target_service})
        );

        // 1. Charger le Mandant
        let mandator_result = manager
            .get_document("mandators", &mandator_id.to_string())
            .await;

        let mandator_doc = match mandator_result {
            Ok(Some(doc)) => doc,
            Ok(None) => raise_error!(
                "ERR_RBAC_MANDATOR_NOT_FOUND",
                context = json_value!({"mandator_id": mandator_id})
            ),
            Err(e) => raise_error!(
                "ERR_RBAC_DB_ACCESS",
                error = e.to_string(),
                context = json_value!({"mandator_id": mandator_id})
            ),
        };

        let mandator: Mandator = match json::deserialize_from_value(mandator_doc) {
            Ok(m) => m,
            Err(e) => raise_error!(
                "ERR_RBAC_MANDATOR_CORRUPT",
                error = e.to_string(),
                context = json_value!({"mandator_id": mandator_id})
            ),
        };

        if mandator.status != "ACTIVE" {
            raise_error!(
                "ERR_RBAC_MANDATOR_INACTIVE",
                context = json_value!({"status": mandator.status})
            );
        }

        // 2. Parcourir les rôles
        for role_id in &mandator.assigned_roles {
            let role_result = manager.get_document("roles", &role_id.to_string()).await;
            let role_doc = match role_result {
                Ok(Some(doc)) => doc,
                Ok(None) => continue, // Rôle introuvable, on l'ignore silencieusement
                Err(e) => {
                    user_warn!(
                        "RBAC_ROLE_DB_ERROR",
                        json_value!({"role_id": role_id, "error": e.to_string()})
                    );
                    continue;
                }
            };

            let role: Role = match json::deserialize_from_value(role_doc) {
                Ok(r) => r,
                Err(e) => {
                    user_warn!(
                        "RBAC_ROLE_CORRUPT",
                        json_value!({"role_id": role_id, "error": e.to_string()})
                    );
                    continue;
                }
            };

            if role.status != "ACTIVE" {
                continue;
            }

            // 3. Parcourir les permissions
            for perm_id in &role.granted_permissions {
                let perm_result = manager
                    .get_document("permissions", &perm_id.to_string())
                    .await;
                let perm_doc = match perm_result {
                    Ok(Some(doc)) => doc,
                    Ok(None) => continue,
                    Err(_) => continue,
                };

                let perm: Permission = match json::deserialize_from_value(perm_doc) {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                if perm.service == target_service && perm.action == target_action {
                    // 4. Évaluation des conditions dynamiques (AST)
                    if let Some(conditions_ast) = &perm.conditions {
                        let is_condition_met =
                            Self::evaluate_condition(conditions_ast, evaluation_context).await;
                        if !is_condition_met {
                            user_debug!(
                                "RBAC_CONDITION_NOT_MET",
                                json_value!({"permission": perm.handle})
                            );
                            continue;
                        }
                    }

                    user_success!(
                        "RBAC_ACCESS_GRANTED",
                        json_value!({"permission": perm.handle})
                    );
                    return Ok(());
                }
            }
        }

        // 5. Rejet explicite si aucune permission n'a validé l'accès
        raise_error!(
            "ERR_RBAC_ACCESS_DENIED",
            context = json_value!({
                "mandator_id": mandator_id,
                "service": target_service,
                "action": format!("{:?}", target_action)
            })
        );

        #[allow(unreachable_code)]
        Ok(())
    }

    /// Évaluation stricte de l'AST via le rules_engine
    async fn evaluate_condition(ast_json: &JsonValue, context: &JsonValue) -> bool {
        let expr = match json::deserialize_from_value::<Expr>(ast_json.clone()) {
            Ok(e) => e,
            Err(_) => return false,
        };

        let eval_result = match Evaluator::evaluate(&expr, context, &NoOpDataProvider).await {
            Ok(res) => res,
            Err(_) => return false,
        };

        match eval_result.as_ref() {
            JsonValue::Bool(b) => *b,
            _ => false,
        }
    }

    /// ROW-LEVEL SECURITY (RLS) : Extrait la politique de sécurité sous forme d'AST
    pub async fn get_read_policy_ast(
        manager: &CollectionsManager<'_>,
        mandator_id: &UniqueId,
        target_resource: &str, // ex: "missions"
    ) -> RaiseResult<Option<Expr>> {
        let mandator_doc = manager
            .get_document("mandators", &mandator_id.to_string())
            .await?
            .ok_or_else(|| build_error!("ERR_RBAC_MANDATOR_NOT_FOUND"))?;

        let mandator: Mandator = json::deserialize_from_value(mandator_doc).unwrap();
        if mandator.status != "ACTIVE" {
            raise_error!("ERR_RBAC_MANDATOR_INACTIVE");
        }

        let mut allowed_asts = Vec::new();
        let mut has_unconditional_access = false;

        for role_id in &mandator.assigned_roles {
            if let Ok(Some(role_doc)) = manager.get_document("roles", &role_id.to_string()).await {
                let role: Role = json::deserialize_from_value(role_doc).unwrap();
                if role.status != "ACTIVE" {
                    continue;
                }

                for perm_id in &role.granted_permissions {
                    if let Ok(Some(perm_doc)) = manager
                        .get_document("permissions", &perm_id.to_string())
                        .await
                    {
                        // On cherche spécifiquement les permissions de LECTURE sur la ressource demandée
                        if perm_doc["resource"] == target_resource && perm_doc["action"] == "read" {
                            if let Some(conditions_ast) = perm_doc.get("conditions") {
                                if let Ok(expr) =
                                    json::deserialize_from_value::<Expr>(conditions_ast.clone())
                                {
                                    allowed_asts.push(expr);
                                }
                            } else {
                                // Une permission sans condition donne un accès absolu !
                                has_unconditional_access = true;
                            }
                        }
                    }
                }
            }
        }

        // Si l'utilisateur est un super-admin de cette ressource (accès inconditionnel)
        if has_unconditional_access {
            return Ok(None);
        }

        // S'il n'a aucune permission de lecture
        if allowed_asts.is_empty() {
            raise_error!(
                "ERR_RBAC_ACCESS_DENIED",
                context = json_value!({"resource": target_resource, "action": "read"})
            );
        }

        // Fusion intelligente : S'il a plusieurs rôles, on les combine avec un OU
        if allowed_asts.len() == 1 {
            Ok(Some(allowed_asts.pop().unwrap()))
        } else {
            Ok(Some(Expr::Or(allowed_asts)))
        }
    }
}

// ============================================================================
// TESTS UNITAIRES ROBUSTES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    // 🎯 FIX : On importe le bon environnement de test pour la BDD
    use crate::json_db::test_utils::init_test_env;
    use crate::utils::testing::DbSandbox;

    /// Helper : Injecte un graphe RBAC complet dans la base de données de test
    async fn setup_rbac_graph(
        manager: &CollectionsManager<'_>,
        mandator_id: &str,
        role_id: &str,
        perm_id: &str,
        mandator_status: &str,
        ast_condition: Option<JsonValue>,
    ) {
        manager
            .create_collection(
                "permissions",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .create_collection(
                "roles",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .create_collection(
                "mandators",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        manager
            .upsert_document(
                "permissions",
                json_value!({
                    "handle": "perm-test",
                    "_id": perm_id,
                    "name": "Execute Action",
                    "service": "test_service",
                    "action": "EXECUTE",
                    "conditions": ast_condition
                }),
            )
            .await
            .unwrap();

        manager
            .upsert_document(
                "roles",
                json_value!({
                    "handle": "role-test",
                    "_id": role_id,
                    "name": "Test Role",
                    "granted_permissions": [perm_id],
                    "inherited_roles": [],
                    "status": "ACTIVE"
                }),
            )
            .await
            .unwrap();

        manager.upsert_document("mandators", json_value!({
            "handle": "mandator-test",
            "_id": mandator_id,
            "nature": "HUMAN",
            "user_ids": [],
            "assigned_roles": [role_id],
            "authority_scope": {"organizations":[], "domains":[], "teams":[], "databases":[]},
            "authorized_layers": ["SA"],
            "public_key": "xxx",
            "status": mandator_status
        })).await.unwrap();
    }

    #[async_test]
    async fn test_rbac_verify_access_success_no_conditions() -> RaiseResult<()> {
        // 🎯 FIX : Initialisation propre du système DB de Raise
        let env = init_test_env().await?;
        let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
        DbSandbox::mock_db(&manager).await.unwrap();

        let m_id = "00000000-0000-0000-0000-000000000001";
        let r_id = "00000000-0000-0000-0000-000000000002";
        let p_id = "00000000-0000-0000-0000-000000000003";

        setup_rbac_graph(&manager, m_id, r_id, p_id, "ACTIVE", None).await;

        let result = RbacEngine::verify_access(
            &manager,
            &UniqueId::try_from(m_id).unwrap(),
            "test_service",
            ActionType::Execute,
            &json_value!({}),
        )
        .await;

        assert!(result.is_ok(), "L'accès doit être accordé.");

        Ok(())
    }

    #[async_test]
    async fn test_rbac_verify_access_denied_wrong_action() -> RaiseResult<()> {
        let env = init_test_env().await?;
        let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
        DbSandbox::mock_db(&manager).await?;

        let m_id = "00000000-0000-0000-0000-000000000001";
        let r_id = "00000000-0000-0000-0000-000000000002";
        let p_id = "00000000-0000-0000-0000-000000000003";

        setup_rbac_graph(&manager, m_id, r_id, p_id, "ACTIVE", None).await;

        let result = RbacEngine::verify_access(
            &manager,
            &UniqueId::try_from(m_id).unwrap(),
            "test_service",
            ActionType::Delete,
            &json_value!({}),
        )
        .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("ERR_RBAC_ACCESS_DENIED"));

        Ok(())
    }

    #[async_test]
    async fn test_rbac_verify_access_mandator_inactive() -> RaiseResult<()> {
        let env = init_test_env().await?;
        let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
        DbSandbox::mock_db(&manager).await?;

        let m_id = "00000000-0000-0000-0000-000000000001";
        let r_id = "00000000-0000-0000-0000-000000000002";
        let p_id = "00000000-0000-0000-0000-000000000003";

        setup_rbac_graph(&manager, m_id, r_id, p_id, "SUSPENDED", None).await;

        let result = RbacEngine::verify_access(
            &manager,
            &UniqueId::try_from(m_id).unwrap(),
            "test_service",
            ActionType::Execute,
            &json_value!({}),
        )
        .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("ERR_RBAC_MANDATOR_INACTIVE"));

        Ok(())
    }

    #[async_test]
    async fn test_rbac_verify_access_with_dynamic_ast_success() -> RaiseResult<()> {
        let env = init_test_env().await?;
        let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
        DbSandbox::mock_db(&manager).await?;

        let m_id = "00000000-0000-0000-0000-000000000001";
        let r_id = "00000000-0000-0000-0000-000000000002";
        let p_id = "00000000-0000-0000-0000-000000000003";

        let ast_condition = json_value!({ "eq": [{"var": "mission_status"}, {"val": "DRAFT"}] });
        setup_rbac_graph(&manager, m_id, r_id, p_id, "ACTIVE", Some(ast_condition)).await;

        let context = json_value!({"mission_status": "DRAFT"});

        let result = RbacEngine::verify_access(
            &manager,
            &UniqueId::try_from(m_id).unwrap(),
            "test_service",
            ActionType::Execute,
            &context,
        )
        .await;

        assert!(result.is_ok(), "L'AST est validé, accès accordé.");

        Ok(())
    }

    #[async_test]
    async fn test_rbac_verify_access_with_dynamic_ast_failure() -> RaiseResult<()> {
        let env = init_test_env().await?;
        let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
        DbSandbox::mock_db(&manager).await?;

        let m_id = "00000000-0000-0000-0000-000000000001";
        let r_id = "00000000-0000-0000-0000-000000000002";
        let p_id = "00000000-0000-0000-0000-000000000003";

        let ast_condition = json_value!({ "eq": [{"var": "mission_status"}, {"val": "DRAFT"}] });
        setup_rbac_graph(&manager, m_id, r_id, p_id, "ACTIVE", Some(ast_condition)).await;

        let bad_context = json_value!({"mission_status": "ACTIVE"});

        let result = RbacEngine::verify_access(
            &manager,
            &UniqueId::try_from(m_id).unwrap(),
            "test_service",
            ActionType::Execute,
            &bad_context,
        )
        .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("ERR_RBAC_ACCESS_DENIED"),
            "L'AST a échoué, l'accès doit être refusé."
        );

        Ok(())
    }
}
