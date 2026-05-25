// FICHIER : src-tauri/src/utils/net/net.rs

// 1. Core : Concurrence, Temps et Erreurs
use crate::utils::core::error::RaiseResult;
use crate::utils::core::{sleep_async, StaticCell, TimeDuration};

// 2. Data : Sérialisation et JSON
use crate::utils::data::json::json_value;
use crate::utils::data::{DeserializableOwned, Serializable};

// 3. Network : Types HTTP (via la façade network/mod.rs)
use crate::utils::network::http_types::{HttpClient, HttpClientBuilder, HttpStatusCode};

/// Singleton : Le client HTTP est réutilisé pour bénéficier du pool de connexions (Performance).
static GLOBAL_CLIENT: StaticCell<HttpClient> = StaticCell::new();

/// Récupère l'instance unique du client HTTP global.
pub fn get_client() -> &'static HttpClient {
    GLOBAL_CLIENT.get_or_init(|| {
        HttpClientBuilder::new()
            .timeout(TimeDuration::from_secs(60))
            .pool_idle_timeout(TimeDuration::from_secs(90))
            .user_agent(concat!("Raise-Core/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("❌ CRITICAL: Impossible d'initialiser le client HTTP global")
    })
}

/// Envoie une requête POST JSON avec Authentification Bearer optionnelle et stratégie de Retry.
pub async fn post_authenticated_async<T: Serializable, R: DeserializableOwned>(
    url: &str,
    body: &T,
    token: Option<&str>,
    max_retries: u32,
) -> RaiseResult<R> {
    let client = get_client();
    let mut attempt = 0;
    let mut delay = TimeDuration::from_secs(1);

    loop {
        attempt += 1;

        let mut request_builder = client.post(url).json(body);

        if let Some(tk) = token {
            request_builder = request_builder.header("Authorization", format!("Bearer {}", tk));
        }

        crate::user_debug!(
            "NET_POST_ATTEMPT",
            json_value!({ "url": url, "attempt": attempt, "max_retries": max_retries })
        );

        match request_builder.send().await {
            Ok(response) => {
                let status = response.status();

                if status.is_success() {
                    return match response.json::<R>().await {
                        Ok(data) => Ok(data),
                        Err(e) => {
                            crate::raise_error!(
                                "ERR_NET_JSON_DECODE",
                                error = e,
                                context = json_value!({ "url": url, "attempt": attempt })
                            )
                        }
                    };
                }

                // 🎯 Utilisation STRICTE de la macro métier Raise au lieu de tracing::warn!
                crate::user_warn!(
                    "NET_HTTP_ERROR",
                    json_value!({ "url": url, "status": status.as_u16(), "attempt": attempt })
                );

                if status.is_client_error() && status != HttpStatusCode::TOO_MANY_REQUESTS {
                    let http_err = response.error_for_status().unwrap_err();
                    crate::raise_error!(
                        "ERR_NET_HTTP_CLIENT_FATAL",
                        error = http_err,
                        context = json_value!({
                            "url": url,
                            "status": status.as_u16(),
                            "attempt": attempt
                        })
                    );
                }
            }
            Err(e) => {
                // 🎯 Utilisation STRICTE de la macro métier Raise
                crate::user_warn!(
                    "NET_CONN_FAILED",
                    json_value!({ "url": url, "attempt": attempt, "error": e.to_string() })
                );
            }
        }

        if attempt >= max_retries {
            crate::raise_error!(
                "ERR_NET_MAX_RETRIES",
                error = "Le service ne répond pas ou renvoie des erreurs persistantes",
                context = json_value!({
                    "url": url,
                    "max_retries": max_retries
                })
            );
        }

        sleep_async(delay).await;
        delay = delay.min(TimeDuration::from_secs(10));
    }
}

/// Helper pour les appels POST JSON sans authentification avec retry.
pub async fn post_json_with_retry_async<T: Serializable, R: DeserializableOwned>(
    url: &str,
    body: &T,
    max_retries: u32,
) -> RaiseResult<R> {
    post_authenticated_async(url, body, None, max_retries).await
}

/// Effectue une requête GET simple et retourne le corps en String.
pub async fn get_string_async(url: &str) -> RaiseResult<String> {
    let client = get_client();

    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => crate::raise_error!(
            "ERR_NET_GET_SEND",
            error = e,
            context = json_value!({ "url": url })
        ),
    };

    let resp = match resp.error_for_status() {
        Ok(r) => r,
        Err(e) => crate::raise_error!(
            "ERR_NET_GET_STATUS",
            error = e,
            context = json_value!({ "url": url, "status": e.status().map(|s| s.as_u16()) })
        ),
    };

    match resp.text().await {
        Ok(t) => Ok(t),
        Err(e) => crate::raise_error!(
            "ERR_NET_GET_TEXT",
            error = e,
            context = json_value!({ "url": url })
        ),
    }
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::core::async_test;
    use crate::utils::core::error::AppError;
    use crate::utils::core::is_same_reference;

    #[test]
    fn test_client_singleton_is_stable() {
        let c1 = get_client();
        let c2 = get_client();
        // 🎯 is_same_reference remplace std::ptr::eq
        assert!(is_same_reference(c1, c2));
    }

    #[async_test]
    async fn test_network_error_observability() {
        let res = get_string_async("http://0.0.0.0:1").await;

        assert!(res.is_err());

        // 🎯 On s'assure qu'on récupère bien la structure AppError de Raise
        if let Err(AppError::Structured(data)) = res {
            assert_eq!(data.code, "ERR_NET_GET_SEND");
            assert_eq!(data.component, "CLIENT");
        } else {
            panic!("L'erreur devrait être de type AppError::Structured");
        }
    }
}
