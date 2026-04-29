// FICHIER : src-tauri/src/utils/network/mod.rs

pub mod client;
pub mod p2p;
pub mod server;

// =========================================================================
// FAÇADE `network` : Réseau et Connectivité (AI-Ready Strict)
// =========================================================================

pub mod http_types {
    // --- Client HTTP (Reqwest) ---
    /// 🤖 IA NOTE : Le moteur HTTP interne pour les requêtes sortantes.
    pub use reqwest::Client as HttpClient;
    /// 🤖 IA NOTE : Constructeur pour configurer le client HTTP global.
    pub use reqwest::ClientBuilder as HttpClientBuilder;
    /// 🤖 IA NOTE : Code de statut HTTP (200, 404, etc.).
    pub use reqwest::StatusCode as HttpStatusCode;

    // --- Serveur HTTP (Axum / Tokio) ---
    /// 🤖 IA NOTE : Extracteur de payload JSON pour les requêtes entrantes.
    pub use axum::extract::Json as HttpJsonPayload;
    /// 🤖 IA NOTE : Lanceur du serveur HTTP asynchrone.
    pub use axum::serve as run_http_server;
    /// 🤖 IA NOTE : Le routeur principal pour définir les endpoints de l'API REST locale.
    pub use axum::Router as HttpRouter;
    /// 🤖 IA NOTE : Écouteur réseau TCP pour le serveur.
    pub use tokio::net::TcpListener as HttpTcpListener;
}

pub mod p2p_types {
    pub use libp2p::swarm::NetworkBehaviour as InternalDerive;
    pub use libp2p::{
        connection_limits, gossipsub, identity, kad, request_response, StreamProtocol,
    }; // Pour la macro
       // --- Identité et Adressage ---
    /// 🤖 IA NOTE : Identité cryptographique locale du nœud (génération de clés Ed25519).
    pub use libp2p::identity as P2pIdentity;
    /// 🤖 IA NOTE : Format d'adresse réseau composable (ex: "/ip4/127.0.0.1/tcp/8080").
    pub use libp2p::Multiaddr as P2pMultiaddr;
    /// 🤖 IA NOTE : Identifiant cryptographique unique d'un nœud sur le réseau.
    pub use libp2p::PeerId as P2pPeerId;

    // --- Cœur du Réseau (Swarm & Transport) ---
    /// 🤖 IA NOTE : Énumération de tous les événements produits par le Swarm (connexions, messages, erreurs).
    pub use libp2p::swarm::SwarmEvent as P2pSwarmEvent; // 🎯 L'ajout indispensable !
    /// 🤖 IA NOTE : Le gestionnaire central du réseau P2P qui orchestre les connexions.
    pub use libp2p::Swarm as P2pSwarm;
    /// 🤖 IA NOTE : Constructeur pour initialiser le P2pSwarm.
    pub use libp2p::SwarmBuilder as P2pSwarmBuilder;
    /// 🤖 IA NOTE : Trait représentant les protocoles de transport (TCP, QUIC).
    pub use libp2p::Transport as P2pTransportTrait;

    // --- Comportements (Behaviours) & Protocoles ---
    /// 🤖 IA NOTE : Module gérant les limites de connexions entrantes/sortantes.
    pub use libp2p::connection_limits as P2pConnectionLimits;
    /// 🤖 IA NOTE : Moteur de messagerie Pub/Sub (Gossip) pour diffuser des messages.
    pub use libp2p::gossipsub as P2pGossipSub;
    /// 🤖 IA NOTE : Table de routage distribuée (DHT) pour la découverte des nœuds.
    pub use libp2p::kad as P2pKademlia;
    /// 🤖 IA NOTE : Protocole pour l'envoi de messages directs (RPC) entre deux nœuds.
    pub use libp2p::request_response as P2pRequestResponse;
    /// 🤖 IA NOTE : Trait requis pour combiner plusieurs comportements réseau (Kad, Gossip).
    pub use libp2p::swarm::NetworkBehaviour as P2pNetworkBehaviourTrait;
    /// 🤖 IA NOTE : Alias pour la macro de dérivation du comportement réseau.
    /// Permet d'implémenter automatiquement la gestion des événements P2P.
    pub use libp2p::swarm::NetworkBehaviour as P2pBehaviour; // 🎯 L'alias sémantique strict !
    /// 🤖 IA NOTE : Identifiant de protocole réseau (ex: "/raise/core/1.0.0").
    pub use libp2p::StreamProtocol as P2pStreamProtocol;

    // --- Sécurité & Multiplexage ---
    /// 🤖 IA NOTE : Protocole de chiffrement pour sécuriser le tunnel P2P (Noise).
    pub use libp2p::noise as P2pNoise;
    /// 🤖 IA NOTE : Protocole de multiplexage pour faire passer plusieurs flux sur une seule connexion TCP (Yamux).
    pub use libp2p::yamux as P2pYamux;
}

// --- Exports Métier Haut Niveau ---
// Les fonctions prêtes à l'emploi que le reste de l'application (et l'IA) doit utiliser.
pub use client::{
    get_client, get_string_async, post_authenticated_async, post_json_with_retry_async,
};
pub use p2p::build_p2p_node_async;
pub use server::start_local_api_async;
