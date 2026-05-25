// FICHIER : src-tauri/src/utils/network/server.rs

use crate::utils::core::error::RaiseResult;

// 2. Data : JSON
use crate::utils::data::json::json_value;

// 3. Network : Types Serveur (via la façade network/mod.rs)
use crate::utils::network::http_types::{run_http_server, HttpRouter, HttpTcpListener};

/// Lance un serveur HTTP local de manière asynchrone sur le port spécifié.
/// 🤖 IA NOTE : Utilisez cette fonction pour exposer des endpoints REST métier.
pub async fn start_local_api_async(port: u16, router: HttpRouter) -> RaiseResult<()> {
    let addr = format!("127.0.0.1:{}", port);

    // 🎯 Utilisation stricte de l'alias HttpTcpListener
    let listener = match HttpTcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => crate::raise_error!(
            "ERR_NETWORK_SERVER_BIND",
            error = e,
            context = json_value!({ "port": port, "address": addr })
        ),
    };

    crate::user_info!(
        "NETWORK_SERVER_STARTED",
        json_value!({ "port": port, "url": format!("http://{}", addr) })
    );

    // 🎯 Lancement avec l'alias et capture d'erreur "Raise"
    if let Err(e) = run_http_server(listener, router).await {
        crate::raise_error!(
            "ERR_NETWORK_SERVER_CRASH",
            error = e,
            context = json_value!({ "port": port })
        );
    }

    Ok(())
}
