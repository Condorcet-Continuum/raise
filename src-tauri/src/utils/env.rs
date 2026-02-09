use crate::utils::{AppError, Result};
use std::env;
use std::str::FromStr;

/// Récupère une variable d'environnement (Requis).
/// Renvoie une erreur explicite si la clé est manquante.
pub fn get(key: &str) -> Result<String> {
    env::var(key)
        .map_err(|_| AppError::Config(format!("Variable d'environnement manquante : {}", key)))
}

/// Récupère une variable d'environnement (Optionnel).
/// Renvoie `None` si la clé est manquante.
pub fn get_optional(key: &str) -> Option<String> {
    env::var(key).ok()
}

/// Récupère une variable d'environnement avec valeur par défaut.
pub fn get_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Récupère et parse une variable (ex: booléen, entier).
/// Utile pour DEBUG=true ou PORT=8080.
pub fn get_parsed<T: FromStr>(key: &str) -> Result<T> {
    let val = get(key)?;
    val.parse::<T>()
        .map_err(|_| AppError::Config(format!("Impossible de parser la variable : {}", key)))
}

/// Indique si une feature flag est active (ex: "true", "1", "yes").
pub fn is_enabled(key: &str) -> bool {
    matches!(
        get_optional(key).as_deref(),
        Some("true") | Some("1") | Some("yes") | Some("on")
    )
}
