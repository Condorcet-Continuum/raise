// FICHIER : src-tauri/src/utils/net.rs

use crate::utils::Result;
use anyhow::Context;
use reqwest::{Client, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{debug, error, instrument, warn};

// Singleton : On ne crée le client qu'une seule fois pour toute l'application.
static GLOBAL_CLIENT: OnceLock<Client> = OnceLock::new();

/// Récupère l'instance unique du client HTTP.
pub fn get_client() -> &'static Client {
    GLOBAL_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(60)) // Timeout long pour l'IA (génération lente)
            .pool_idle_timeout(Duration::from_secs(90))
            .user_agent(concat!("Raise-Core/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("❌ CRITICAL: Impossible d'initialiser le client HTTP global")
    })
}

/// Envoie une requête POST JSON avec Authentification optionnelle et Retry.
#[instrument(skip(body, token), fields(url = %url))]
pub async fn post_authenticated<T: Serialize, R: DeserializeOwned>(
    url: &str,
    body: &T,
    token: Option<&str>,
    max_retries: u32,
) -> Result<R> {
    let client = get_client();
    let mut attempt = 0;
    let mut delay = Duration::from_secs(1);

    loop {
        attempt += 1;

        // Construction de la requête
        let mut request_builder = client.post(url).json(body);

        // Injection du Token si présent (Bearer)
        if let Some(tk) = token {
            request_builder = request_builder.header("Authorization", format!("Bearer {}", tk));
        }

        debug!("Tentative POST {}/{} vers {}", attempt, max_retries, url);

        match request_builder.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    let data = response
                        .json::<R>()
                        .await
                        .context("Erreur de désérialisation de la réponse API")?;
                    return Ok(data);
                }

                warn!("Erreur HTTP {} sur {}", status, url);

                // Erreurs fatales (401 Unauthorized, 400 Bad Request) : Pas de Retry
                if status == StatusCode::UNAUTHORIZED || status == StatusCode::BAD_REQUEST {
                    let text = response.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!("Erreur Fatale API ({}): {}", status, text).into());
                }
            }
            Err(e) => warn!("Échec connexion : {}", e),
        }

        if attempt >= max_retries {
            error!("Abandon après {} tentatives sur {}", max_retries, url);
            return Err(anyhow::anyhow!("Service indisponible : {}", url).into());
        }

        tokio::time::sleep(delay).await;
        delay = std::cmp::min(delay * 2, Duration::from_secs(10)); // Cap du backoff à 10s
    }
}

/// Helper pour les appels simples sans auth (compatibilité existante)
pub async fn post_json_with_retry<T: Serialize, R: DeserializeOwned>(
    url: &str,
    body: &T,
    max_retries: u32,
) -> Result<R> {
    post_authenticated(url, body, None, max_retries).await
}

/// Effectue un GET simple.
#[instrument]
pub async fn get_simple(url: &str) -> Result<String> {
    let client = get_client();
    let resp = client.get(url).send().await?;

    if !resp.status().is_success() {
        return Err(anyhow::anyhow!("Erreur GET {}: {}", url, resp.status()).into());
    }

    Ok(resp.text().await?)
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_singleton_initialization() {
        // On vérifie que les appels successifs renvoient bien le même objet configuré
        let client1 = get_client();
        let client2 = get_client();

        // Astuce : On compare la représentation Debug car Client n'implémente pas Eq
        // Cela garantit au moins qu'ils ont la même config.
        assert_eq!(format!("{:?}", client1), format!("{:?}", client2));
    }

    #[tokio::test]
    async fn test_bad_url_handling() {
        // Test "Smoke" : On vérifie que le client ne panique pas sur une URL invalide
        // Il doit retourner une erreur propre (Result::Err)
        let res = get_simple("http://localhost:54321/ghost").await;

        assert!(
            res.is_err(),
            "Devrait échouer proprement sur une URL inexistante"
        );

        // On peut vérifier que c'est bien une erreur réseau ou système
        let err = res.unwrap_err();
        println!("Erreur capturée (attendu) : {}", err);
    }
}
