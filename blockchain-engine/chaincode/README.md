# 📄 blockchain-engine/chaincode/README.md

## 📌 Présentation

Le module `chaincode` est l'implémentation du serveur gRPC pour le moteur blockchain de **Raise**. Il contient la logique d'exécution des transactions, la gestion de l'état (State Database) et l'interface de service qui répond aux requêtes provenant de l'application cliente (Tauri).

## ⚙️ Rôle dans l'Architecture

Ce module agit comme un "Smart Contract" autonome. Il consomme les définitions de types de `raise-shared` et expose les services définis dans les fichiers `.proto`.

## 🛠️ Stack Technique

- **Runtime** : `tokio` (Asynchrone haute performance).
- **Serveur gRPC** : `tonic` (Version 0.14.3, alignée sur le workspace).
- **Dépendances Internes** : `raise-shared` (pour les types et les traits de service).

## 🚀 Fonctionnement du Serveur

### Implémentation des Services

Le serveur implémente les traits générés par `tonic`. Chaque fonction de transaction suit généralement ce schéma :

1. Réception d'une requête (Request).
2. Validation de la signature ou des droits.
3. Interaction avec la couche de persistance.
4. Retour d'une réponse structurée (Response).

### Point d'entrée (Main)

Le fichier `src/main.rs` configure le serveur :

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let service = MyChaincode::default();

    Server::builder()
        .add_service(ChaincodeServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}

```

## ⚠️ Notes de Maintenance

- **Conflit de Build** : Comme détaillé dans le README du module `shared`, l'outil `tonic-build` peut rencontrer des limitations de fonctionnalités dans le workspace. En cas d'erreur de compilation sur `configure()`, se référer au rapport d'incident sur l'alignement des versions gRPC.
- **Sécurité** : Le serveur est actuellement configuré pour une écoute locale. Pour la production, l'activation de la feature `tls-ring` ou `tls-webpki-roots` (déjà présentes dans le `Cargo.toml`) est requise.

## 🧪 Tests Unitaires

Chaque fichier source intègre ses propres tests unitaires pour valider la logique métier hors contexte réseau.

```bash
cargo test -p raise-chaincode

```

---
