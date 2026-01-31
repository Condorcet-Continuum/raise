# 📄 blockchain-engine/shared/README.md

## 📌 Présentation

Ce module constitue le socle commun du moteur blockchain de **Raise**. Il gère la communication gRPC en centralisant les définitions de services et les structures de données partagées entre le **Chaincode** (Serveur) et l'interface **Tauri** (Client).

## 🏗️ Architecture Technique

Le module s'appuie sur la pile technologique suivante :

- **Protobuf** : Définition des interfaces dans le dossier `/protos`.
- **Tonic (0.14.3)** : Implémentation gRPC alignée sur le runtime de l'application principale.
- **Prost (0.13)** : Sérialisation des messages.

## ⚠️ Contraintes de Compilation (Workspace)

Le partage de ce module au sein du workspace **Raise** impose une discipline stricte sur les dépendances pour éviter les conflits de types :

1. **Alignement des versions** : La version de `tonic` doit être identique à celle utilisée dans `src-tauri` pour permettre l'unification des bibliothèques par Cargo.
2. **Gestion des "Features"** :

- Le module `shared` active la feature `prost` pour générer les codecs.
- Comme les features sont additives en Rust, cela active implicitement `prost` pour `src-tauri`.

3. **Build Script** : Le fichier `build.rs` utilise `tonic-build` pour compiler les fichiers `.proto` au moment du build.

## 🛠️ Utilisation

### Ajouter ou modifier un service

1. Modifiez le fichier `protos/chaincode.proto`.
2. Lancez une vérification pour régénérer les stubs :

```bash
cargo check -p raise-shared

```

### Accès au code généré

Le code généré est automatiquement inclus via la macro `include_proto!` dans `src/lib.rs`.

---
