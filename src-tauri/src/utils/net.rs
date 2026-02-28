// FICHIER : src-tauri/src/utils/net.rs

use crate::raise_error;
use crate::utils::RaiseResult;
use reqwest::{Client, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{debug, instrument, warn};

/// Singleton : Le client HTTP est réutilisé pour bénéficier du pool de connexions (Performance).
static GLOBAL_CLIENT: OnceLock<Client> = OnceLock::new();

/// Récupère l'instance unique du client HTTP global.
pub fn get_client() -> &'static Client {
    GLOBAL_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(60)) // Timeout généreux pour les réponses LLM
            .pool_idle_timeout(Duration::from_secs(90))
            .user_agent(concat!("Raise-Core/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("❌ CRITICAL: Impossible d'initialiser le client HTTP global")
    })
}

/// Envoie une requête POST JSON avec Authentification Bearer optionnelle et stratégie de Retry.
#[instrument(skip(body, token), fields(url = %url))]
pub async fn post_authenticated<T: Serialize, R: DeserializeOwned>(
    url: &str,
    body: &T,
    token: Option<&str>,
    max_retries: u32,
) -> RaiseResult<R> {
    let client = get_client();
    let mut attempt = 0;
    let mut delay = Duration::from_secs(1);

    loop {
        attempt += 1;

        // Construction de la requête à chaque tentative
        let mut request_builder = client.post(url).json(body);

        if let Some(tk) = token {
            request_builder = request_builder.header("Authorization", format!("Bearer {}", tk));
        }

        debug!("Requête POST {}/{} vers {}", attempt, max_retries, url);

        match request_builder.send().await {
            Ok(response) => {
                let status = response.status();

                if status.is_success() {
                    // Désérialisation avec interception d'erreur structurée
                    return match response.json::<R>().await {
                        Ok(data) => Ok(data),
                        Err(e) => raise_error!(
                            "ERR_NET_JSON_DECODE",
                            error = e,
                            context = serde_json::json!({ "url": url, "attempt": attempt })
                        ),
                    };
                }

                warn!("Erreur HTTP {} sur {} (Tentative {})", status, url, attempt);

                // GESTION DES ERREURS FATALES (Pas de retry pour 401, 403, 400)
                if status.is_client_error() && status != StatusCode::TOO_MANY_REQUESTS {
                    let http_err = response.error_for_status().unwrap_err();
                    raise_error!(
                        "ERR_NET_HTTP_CLIENT_FATAL",
                        error = http_err,
                        context = serde_json::json!({
                            "url": url,
                            "status": status.as_u16(),
                            "attempt": attempt
                        })
                    );
                }
            }
            Err(e) => {
                warn!(
                    "Échec de connexion (Tentative {}/{}): {}",
                    attempt, max_retries, e
                );
            }
        }

        // Vérification de la limite de tentatives
        if attempt >= max_retries {
            raise_error!(
                "ERR_NET_MAX_RETRIES",
                error = "Le service ne répond pas ou renvoie des erreurs persistantes",
                context = serde_json::json!({
                    "url": url,
                    "max_retries": max_retries
                })
            );
        }

        // Backoff exponentiel avant la prochaine tentative
        tokio::time::sleep(delay).await;
        delay = std::cmp::min(delay * 2, Duration::from_secs(10));
    }
}

/// Helper pour les appels POST JSON sans authentification avec retry.
pub async fn post_json_with_retry<T: Serialize, R: DeserializeOwned>(
    url: &str,
    body: &T,
    max_retries: u32,
) -> RaiseResult<R> {
    post_authenticated(url, body, None, max_retries).await
}

/// Effectue une requête GET simple et retourne le corps en String.
#[instrument]
pub async fn get_simple(url: &str) -> RaiseResult<String> {
    let client = get_client();

    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => raise_error!(
            "ERR_NET_GET_SEND",
            error = e,
            context = serde_json::json!({ "url": url })
        ),
    };

    let resp = match resp.error_for_status() {
        Ok(r) => r,
        Err(e) => raise_error!(
            "ERR_NET_GET_STATUS",
            error = e,
            context = serde_json::json!({ "url": url, "status": e.status().map(|s| s.as_u16()) })
        ),
    };

    match resp.text().await {
        Ok(t) => Ok(t),
        Err(e) => raise_error!(
            "ERR_NET_GET_TEXT",
            error = e,
            context = serde_json::json!({ "url": url })
        ),
    }
}

// --- TESTS UNITAIRES (RAISE standard) ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::prelude::*; // Utilisation du prélude pour AppError, etc.

    #[test]
    fn test_client_singleton_is_stable() {
        let c1 = get_client();
        let c2 = get_client();
        // Vérification par pointeur
        assert!(std::ptr::eq(c1, c2));
    }

    #[tokio::test]
    async fn test_network_error_observability() {
        // Test sur une URL invalide pour vérifier la levée d'erreur structurée
        let res = get_simple("http://0.0.0.0:1").await;

        assert!(res.is_err());

        if let Err(AppError::Structured(data)) = res {
            assert_eq!(data.code, "ERR_NET_GET_SEND");
            assert_eq!(data.component, "NET"); // Vérifie la déduction auto du composant
        } else {
            panic!("L'erreur devrait être de type AppError::Structured");
        }
    }
}
