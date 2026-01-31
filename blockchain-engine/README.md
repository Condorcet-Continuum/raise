## 🌐 Vision Globale

Le dossier `blockchain-engine` regroupe l'ensemble des composants nécessaires à la couche de confiance et de persistance distribuée de **Raise**. Il est conçu pour fonctionner comme un service indépendant (Chaincode) capable de communiquer de manière sécurisée et performante avec l'interface utilisateur via le protocole **gRPC**.

## 🏗️ Structure du Projet

Le moteur est divisé en deux modules Rust distincts pour séparer les responsabilités :

| Module          | Rôle                                                                   | Type              |
| --------------- | ---------------------------------------------------------------------- | ----------------- |
| **`shared`**    | Définitions Protobuf, structures de données communes et stubs générés. | Librairie (`lib`) |
| **`chaincode`** | Logique métier du smart contract, serveur gRPC et gestion d'état.      | Binaire (`bin`)   |

## 🔄 Flux de Travail Technique

L'interaction entre les composants suit un cycle de vie strict :

1. **Contrat** : Les services sont définis en `.proto` dans `shared/protos/`.
2. **Génération** : Au moment de la compilation, `shared` génère le code Rust nécessaire.
3. **Implémentation** : `chaincode` implémente ces interfaces pour traiter les données.
4. **Consommation** : `src-tauri` utilise `shared` comme client pour envoyer des commandes au moteur.

## 🛠️ Installation et Prérequis

Pour travailler sur ce moteur, les outils suivants sont nécessaires :

- **Rust & Cargo** (Édition 2021)
- **Protocol Buffers Compiler (`protoc`)** : Indispensable pour la génération de code via `tonic-build`.
- _Linux_ : `sudo apt install protobuf-compiler`
- _Mac_ : `brew install protobuf`

## 🚦 Commandes Utiles

Depuis la racine du projet `blockchain-engine` :

- **Compiler l'ensemble du moteur** :

```bash
cargo build --workspace

```

- **Lancer le serveur de chaincode** :

```bash
cargo run -p raise-chaincode

```

- **Exécuter les tests de logique métier** :

```bash
cargo test --workspace

```

## 📉 État Actuel et Limitations

> [!IMPORTANT]
> Le projet utilise actuellement **Tonic 0.14.3**. En raison de contraintes de synchronisation avec le workspace global de l'application (Tauri v2), des ajustements spécifiques sur les features de compilation sont appliqués pour éviter les conflits de types `Prost`. Consultez les rapports de build en cas d'erreur sur la fonction `configure()`.

---
