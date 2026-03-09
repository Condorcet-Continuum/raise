# 🌐 RAISE Foundation - Module `network` (Connectivité & P2P)

Ce module (`src-tauri/src/utils/network`) est la **seule et unique** porte d'entrée et de sortie réseau de l'application RAISE. Il isole les implémentations lourdes (`reqwest`, `axum`, `libp2p`) derrière une façade sémantique unifiée pour garantir la sécurité et l'observabilité.

---

## 🛑 DIRECTIVES SYSTÈMES STRICTES (POUR LES AGENTS IA)

1. **Bannissement des Crates Externes** : Il est **STRICTEMENT INTERDIT** d'importer directement `reqwest`, `axum`, `tokio::net` ou `libp2p` dans le code métier. Utilisez exclusivement les alias de la façade `http_types` ou `p2p_types`.
2. **Zéro Prelude** : N'utilisez plus `use crate::utils::prelude::*`. Importez explicitement vos besoins depuis `crate::utils::network` ou `crate::utils::core`.
3. **Tests Asynchrones** : Utilisez l'attribut métier `#[async_test]` ré-exporté par le `core` au lieu de l'attribut `tokio` brut.
4. **Le Client est un Singleton** : N'instanciez jamais de `HttpClient`. Utilisez `get_client()` pour bénéficier du pool de connexions global.

---

## 🏗️ Architecture et Dictionnaire (Aliasing)

### 1. HTTP Client & Server (`http_types`)

| Type RAISE | Source Originale (Interdite) | Rôle |
| --- | --- | --- |
| `HttpClient` | `reqwest::Client` | Moteur HTTP interne. |
| `HttpStatusCode` | `reqwest::StatusCode` | Codes de statut (200, 404). |
| `HttpRouter` | `axum::Router` | Définition de l'API locale. |
| `run_http_server` | `axum::serve` | Lancement du serveur. |
| `HttpTcpListener` | `tokio::net::TcpListener` | Écouteur réseau TCP. |

### 2. Réseau Décentralisé (`p2p_types`)

| Type RAISE | Source Originale (Interdite) | Rôle |
| --- | --- | --- |
| `P2pSwarm` | `libp2p::Swarm` | Orchestrateur du nœud. |
| `P2pPeerId` | `libp2p::PeerId` | Identifiant cryptographique. |
| `P2pMultiaddr` | `libp2p::Multiaddr` | Adresse réseau composable. |
| `P2pIdentity` | `libp2p::identity` | Gestion des clés Ed25519. |

---

## 🚀 Fonctions Métier "Ready-to-Use"

### 📡 Client HTTP (`client.rs`)

* `get_string_async(url) -> RaiseResult<String>`
* `post_json_with_retry_async<T, R>(url, body, retries) -> RaiseResult<R>`
* `post_authenticated_async<T, R>(url, body, token, retries) -> RaiseResult<R>`

### 🖥️ Serveur & P2P

* `start_local_api_async(port, router) -> RaiseResult<()>`
* `build_p2p_node_async(behaviour, keypair, port) -> RaiseResult<P2pSwarm<B>>`

---

## 🚨 Standard de Codage RAISE

### ❌ MAUVAIS (Pollution de dépendances)

```rust
use reqwest::Client; // 🛑 INTERDIT
#[tokio::test] // 🛑 INTERDIT
async fn test() { ... }

```

### ✅ BON (Architecture Isclée)

```rust
// Imports explicites depuis les façades
use crate::utils::network::{get_string_async, http_types::HttpRouter};
use crate::utils::core::async_test; // L'attribut de test officiel
use crate::utils::core::error::RaiseResult;

#[async_test]
async fn fetch_example() -> RaiseResult<String> {
    let html = get_string_async("https://raise.ia").await?;
    Ok(html)
}

```

 