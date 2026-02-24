// FICHIER : src-tauri/src/utils/macros.rs

/// Affiche une info à l'utilisateur (traduite) et logue l'événement
#[macro_export]
macro_rules! user_info {
    ($key:expr) => {{
        let msg = $crate::utils::i18n::t($key);
        println!("{}", msg);
        tracing::info!(event = "user_notification", key = $key, message = %msg);
    }};
    ($key:expr, $($arg:tt)*) => {{
        let args_formatted = format!($($arg)*);
        let full_msg = format!("{} {}", $crate::utils::i18n::t($key), args_formatted);
        println!("{}", full_msg);
        tracing::info!(event = "user_notification", key = $key, message = %full_msg);
    }};
}

/// Affiche un succès (vert) à l'utilisateur
#[macro_export]
macro_rules! user_success {
    ($key:expr) => {{
        let msg = $crate::utils::i18n::t($key);
        println!("✅ {}", msg);
        tracing::info!(event = "user_success", key = $key, message = %msg);
    }};
    ($key:expr, $($arg:tt)*) => {{
        let args_formatted = format!($($arg)*);
        let full_msg = format!("{} {}", $crate::utils::i18n::t($key), args_formatted);
        println!("✅ {}", full_msg);
        tracing::info!(event = "user_success", key = $key, message = %full_msg);
    }};
}

/// Affiche une erreur à l'utilisateur ET logue la structure technique enrichie
#[macro_export]
macro_rules! user_error {
    // Cas 1 : Juste une clé (Legacy)
    ($key:expr) => {{
        let msg = $crate::utils::i18n::t($key);
        eprintln!("❌ {}", msg);
        tracing::error!(event = "user_error", key = $key, message = %msg);
    }};

    // =========================================================================
    // NOUVEAUX CAS : Format Enrichi (Type Datadog / ELK)
    // On définit explicitement les 4 combinaisons pour satisfaire le parseur strict.
    // =========================================================================

    // A) La totale : Avec correlation_id ET user_id
    (
        $key:expr,
        error = $err:expr,
        component = $comp:expr,
        action = $action:expr,
        correlation_id = $corr_id:expr,
        user_id = $usr_id:expr
    ) => {{
        let msg = $crate::utils::i18n::t($key);
        eprintln!("❌ [{}] {} : {}", $comp, msg, $err);
        tracing::error!(
            service = "raise-app", appId = "Raise-1.0", componentName = $comp, action = $action,
            reason = %msg, error = ?$err, correlationId = %$corr_id, userId = %$usr_id,
            event = "user_error", key = $key
        );
    }};

    // B) Uniquement correlation_id
    (
        $key:expr,
        error = $err:expr,
        component = $comp:expr,
        action = $action:expr,
        correlation_id = $corr_id:expr
    ) => {{
        let msg = $crate::utils::i18n::t($key);
        eprintln!("❌ [{}] {} : {}", $comp, msg, $err);
        tracing::error!(
            service = "raise-app", appId = "Raise-1.0", componentName = $comp, action = $action,
            reason = %msg, error = ?$err, correlationId = %$corr_id,
            event = "user_error", key = $key
        );
    }};

    // C) Uniquement user_id
    (
        $key:expr,
        error = $err:expr,
        component = $comp:expr,
        action = $action:expr,
        user_id = $usr_id:expr
    ) => {{
        let msg = $crate::utils::i18n::t($key);
        eprintln!("❌ [{}] {} : {}", $comp, msg, $err);
        tracing::error!(
            service = "raise-app", appId = "Raise-1.0", componentName = $comp, action = $action,
            reason = %msg, error = ?$err, userId = %$usr_id,
            event = "user_error", key = $key
        );
    }};

    // D) Format enrichi minimal (Sans correlation_id ni user_id)
    (
        $key:expr,
        error = $err:expr,
        component = $comp:expr,
        action = $action:expr
    ) => {{
        let msg = $crate::utils::i18n::t($key);
        eprintln!("❌ [{}] {} : {}", $comp, msg, $err);
        tracing::error!(
            service = "raise-app", appId = "Raise-1.0", componentName = $comp, action = $action,
            reason = %msg, error = ?$err,
            event = "user_error", key = $key
        );
    }};

    // =========================================================================

    // Cas 3 : Clé + Arguments de formatage (Legacy)
    // (Doit toujours être placé à la fin pour ne pas intercepter les syntaxes du dessus)
    ($key:expr, $($arg:tt)*) => {{
        let args_formatted = format!($($arg)*);
        let full_msg = format!("{} {}", $crate::utils::i18n::t($key), args_formatted);
        eprintln!("❌ {}", full_msg);
        tracing::error!(event = "user_error", key = $key, message = %full_msg);
    }};
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use crate::utils::error::AppError;
    use uuid::Uuid;

    #[test]
    fn test_macro_user_error_enriched_combinations() {
        let sim_err = AppError::Config("Fichier corrompu".to_string());
        let corr_id = Uuid::new_v4().to_string();

        // 1. Test "La Totale"
        user_error!(
            "ERR_CONFIG_LOAD",
            error = sim_err,
            component = "CONFIG_SERVICE",
            action = "PROCESS_LOAD_CONFIG",
            correlation_id = corr_id,
            user_id = "user-123"
        );

        // 2. Test "Minimal" (Sans ID)
        let sim_err_2 = AppError::Config("Port occupé".to_string());
        user_error!(
            "ERR_PORT",
            error = sim_err_2,
            component = "NET_SERVICE",
            action = "PROCESS_BIND"
        );
    }
}
