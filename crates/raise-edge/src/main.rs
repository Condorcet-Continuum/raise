use axum::{routing::get, Router};

#[tokio::main]
async fn main() {
    println!("🚀 R.A.I.S.E. Edge Node - Démarrage de l'agent...");

    // Définition des routes réseau de base pour l'agent
    let app = Router::new().route(
        "/health",
        get(|| async { "Agent Edge Online et Opérationnel" }),
    );

    // Écoute sur toutes les interfaces réseau (0.0.0.0)
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("📡 En écoute sur {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}
