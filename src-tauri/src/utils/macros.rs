// Ces macros doivent être exportées pour être utilisées en dehors du module utils

/// Affiche une info à l'utilisateur (traduite) et logue l'événement (technique)
/// Note: Les arguments suivent la syntaxe standard de format!().
#[macro_export]
macro_rules! user_info {
    // Cas 1 : Juste une clé, pas d'arguments
    ($key:expr) => {{
        let msg = $crate::utils::i18n::t($key);
        println!("{}", msg);
        tracing::info!(event = "user_notification", key = $key, message = %msg);
    }};
    // Cas 2 : Clé + Arguments de formatage
    ($key:expr, $($arg:tt)*) => {{
        // On formate les arguments d'abord (ex: "Fichier {} manquant", "t.json")
        let args_formatted = format!($($arg)*);
        // On concatène la traduction (Prefixe) avec les arguments
        let full_msg = format!("{} {}", $crate::utils::i18n::t($key), args_formatted);

        println!("{}", full_msg);
        // On logue avec les détails
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

/// Affiche une erreur (stderr) à l'utilisateur
#[macro_export]
macro_rules! user_error {
    ($key:expr) => {{
        let msg = $crate::utils::i18n::t($key);
        eprintln!("❌ {}", msg);
        tracing::error!(event = "user_error", key = $key, message = %msg);
    }};
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
    // Ces tests vérifient que la compilation des macros fonctionne.
    // Les appels utilisent maintenant des chaînes de format valides "{}".

    #[test]
    fn test_macro_user_info_compilation() {
        user_info!("TEST_KEY");
        // Syntaxe valide : format string + arg
        user_info!("TEST_KEY", "Valeur: {}", 42);
        // Syntaxe valide : literal string
        user_info!("TEST_KEY", "Juste un message sup");
    }

    #[test]
    fn test_macro_user_success_compilation() {
        user_success!("TEST_KEY");
        user_success!("TEST_KEY", "ID: {}", 123);
    }

    #[test]
    fn test_macro_user_error_compilation() {
        user_error!("TEST_KEY");
        user_error!("TEST_KEY", "Détails: {}", "Timeout");
    }
}
