// FICHIER : src-tauri/src/utils/macros.rs

/// Affiche une info à l'utilisateur (traduite) et logue l'événement
#[macro_export]
macro_rules! user_info {
    ($key:expr) => {{
        let msg = $crate::utils::i18n::t($key);
        tracing::info!(
            target: "user_notification",
            event_id = $key,
            "{}", msg
        );
    }};
    ($key:expr, $context:expr) => {{
        let msg = $crate::utils::i18n::t($key);
        tracing::info!(
            target: "user_notification",
            event_id = $key,
            context = %$context,
            "{}", msg
        );
    }};
}

/// Affiche un succès (vert) à l'utilisateur
#[macro_export]
macro_rules! user_success {
    ($key:expr) => {{
        let msg = $crate::utils::i18n::t($key);
        tracing::info!(
            target: "user_notification",
            event_id = $key,
            severity = "success",
            "✅ {}", msg
        );
    }};
    ($key:expr, $context:expr) => {{
        let msg = $crate::utils::i18n::t($key);
        tracing::info!(
            target: "user_notification",
            event_id = $key,
            severity = "success",
            context = %$context,
            "✅ {}", msg
        );
    }};
}

#[macro_export]
macro_rules! user_error {
    ($key:expr) => {{
        let msg = $crate::utils::i18n::t($key);
        tracing::error!(
            target: "user_notification",
            event_id = $key,
            severity = "error",
            "❌ {}", msg
        );
    }};
    ($key:expr, $context:expr) => {{
        let msg = $crate::utils::i18n::t($key);
        tracing::error!(
            target: "user_notification",
            event_id = $key,
            severity = "error",
            context = %$context,
            "❌ {}", msg
        );
    }};
}

/// Macro surpuissante pour générer des erreurs structurées AI-Ready
#[macro_export]
macro_rules! build_error {
    ($key:expr, error = $err:expr, context = $ctx:expr, correlation_id = $cid:expr, user_id = $uid:expr) => {
        $crate::build_error!(@internal $key, Some($err.to_string()), $ctx, Some($cid.to_string()), Some($uid.to_string()))
    };
    ($key:expr, error = $err:expr, context = $ctx:expr) => {
        $crate::build_error!(@internal $key, Some($err.to_string()), $ctx, None::<String>, None::<String>)
    };
    ($key:expr, error = $err:expr) => {
        $crate::build_error!(@internal $key, Some($err.to_string()), serde_json::json!({}), None::<String>, None::<String>)
    };
    ($key:expr, context = $ctx:expr) => {
        $crate::build_error!(@internal $key, None::<String>, $ctx, None::<String>, None::<String>)
    };
    ($key:expr) => {
        $crate::build_error!(@internal $key, None::<String>, serde_json::json!({}), None::<String>, None::<String>)
    };

    // =========================================================================
    // LE CERVEAU (Interne)
    // =========================================================================
    (@internal $key:expr, $err:expr, $ctx:expr, $corr_id:expr, $usr_id:expr) => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str { std::any::type_name::<T>() }
        let action_name = type_name_of(f).rsplit("::").nth(1).unwrap_or("UNKNOWN_ACTION").to_uppercase();

        let mod_path = module_path!();
        let parts: Vec<&str> = mod_path.split("::").collect();
        let (service, subdomain, component) = match parts.len() {
            0 | 1 => ("core", "general", "unknown"),
            2 => (parts[1], "general", parts[1]),
            3 => (parts[1], "core", parts[2]),
            _ => (parts[1], parts[2], parts[3]),
        };

        let mut ctx_map = serde_json::Map::new();
        ctx_map.insert("action".to_string(), serde_json::json!(action_name));

        if let Some(cid) = $corr_id { ctx_map.insert("correlationId".to_string(), serde_json::json!(cid)); }
        if let Some(uid) = $usr_id { ctx_map.insert("userId".to_string(), serde_json::json!(uid)); }

        // CORRECTION : On injecte l'erreur technique pour ne pas la perdre !
        if let Some(ref e) = $err {
            ctx_map.insert("technical_error".to_string(), serde_json::json!(e));
        }

        // On fusionne le contexte utilisateur
        let context_value = $ctx;
        if let serde_json::Value::Object(user_map) = context_value {
            for (k, v) in user_map { ctx_map.insert(k, v); }
        } else {
            // Si le contexte n'est pas un objet, on le sauvegarde quand même sous "data"
            ctx_map.insert("data".to_string(), context_value);
        }

        let final_context = serde_json::Value::Object(ctx_map);
        let reason_msg = $crate::utils::i18n::t($key);

        tracing::error!(
            event = "user_error",
            key = $key,
            service = %service,
            subdomain = %subdomain,
            componentName = %component.to_uppercase(),
            action = %action_name,
            reason = %reason_msg,
            error = ?$err,
            context = %final_context,
            "❌ [{}] {}", component.to_uppercase(), reason_msg
        );

        $crate::utils::error::AppError::Structured(Box::new($crate::utils::error::StructuredData {
            service: service.to_string(),
            subdomain: subdomain.to_string(),
            component: component.to_uppercase(),
            code: $key.to_string(),
            message: reason_msg,
            context: final_context,
        }))
    }};
}

/// 🚀 Macro de DIVERGENCE (Fait un return Err)
#[macro_export]
macro_rules! raise_error {
    ($($arg:tt)*) => {
        return Err($crate::build_error!($($arg)*))
    };
}

// ============================================================================
// TESTS UNITAIRES DES MACROS
// ============================================================================
#[cfg(test)]
mod tests {
    use crate::utils::error::{AppError, RaiseResult};
    use serde_json::json;

    // Fonction bouchon pour simuler i18n dans les tests (si besoin)
    // On assume que utils::i18n::t(key) retourne au moins la clé si non trouvée.

    #[test]
    fn test_build_error_key_only() {
        let err = crate::build_error!("ERR_SIMPLE");

        let AppError::Structured(data) = err;
        assert_eq!(data.code, "ERR_SIMPLE");
        assert!(
            data.context.get("action").is_some(),
            "L'action doit être auto-détectée"
        );
        assert!(data.context.get("technical_error").is_none());
    }

    #[test]
    fn test_build_error_with_technical_error() {
        let db_err = "Connection refused";
        let err = crate::build_error!("ERR_DB", error = db_err);

        let AppError::Structured(data) = err;
        assert_eq!(data.code, "ERR_DB");

        // Vérification de la correction du "Trou noir"
        assert_eq!(
            data.context["technical_error"].as_str().unwrap(),
            "Connection refused"
        );
    }

    #[test]
    fn test_build_error_with_full_context() {
        let err = crate::build_error!(
            "ERR_API",
            error = "Timeout",
            context = json!({"retry": true, "timeout_ms": 5000})
        );

        let AppError::Structured(data) = err;
        assert_eq!(data.code, "ERR_API");
        assert_eq!(data.context["retry"].as_bool().unwrap(), true);
        assert_eq!(data.context["timeout_ms"].as_i64().unwrap(), 5000);
        assert_eq!(data.context["technical_error"].as_str().unwrap(), "Timeout");
    }

    #[test]
    fn test_raise_error_control_flow() {
        // Cette fonction vérifie que raise_error! fait bien un "return Err(...)"
        fn simulate_failure() -> RaiseResult<i32> {
            crate::raise_error!("ERR_CRITICAL", error = "Crash");
            #[allow(unreachable_code)]
            Ok(42) // Cette ligne ne doit jamais être atteinte
        }

        let result = simulate_failure();
        assert!(result.is_err());

        let AppError::Structured(data) = result.unwrap_err();
        assert_eq!(data.code, "ERR_CRITICAL");
    }
}
