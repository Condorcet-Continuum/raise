# 🏭 GenAptitude Factory - Source WASM

> **L'Usine de fabrication des Blocs Cognitifs**

Ce répertoire (`src-wasm`) est un **Workspace Rust autonome**. C'est ici que sont développés, testés et compilés les modules d'intelligence ("Plugins") avant d'être livrés à l'application principale.

Contrairement au dossier `src-tauri` (qui est le Cerveau/Hôte), ce dossier contient les "Pensées" (Invités) qui seront exécutées dans une sandbox sécurisée via Wasmtime.

---

## 🏗 Architecture de l'Usine

Le workspace est organisé pour séparer l'outillage commun de la logique métier :

1.  **`core-api` (Le Kit de Survie)** :

    - Une librairie Rust partagée.
    - Elle contient les définitions de types (`CognitiveModel`) et surtout les **fonctions systèmes** (`log`, `db_read`).
    - Elle masque la complexité des appels FFI (`unsafe`, pointeurs) pour les développeurs de plugins.

2.  **`blocks/*` (Les Produits)** :

    - Chaque sous-dossier est un plugin indépendant (ex: `spy-plugin`, `analyzer-consistency`).
    - Ils ne connaissent rien de Tauri, ils ne connaissent que `core-api`.

3.  **`build.sh` (La Chaîne de Montage)** :
    - Script d'automatisation qui gère le cycle de vie : **Test ➡️ Compile ➡️ Deploy**.

---

## 📂 Structure du Dossier

```text
src-wasm/
├── Cargo.toml          # Workspace Root (Définit les dépendances partagées : serde, thiserror...)
├── build.sh            # ⚙️ Le script magique de compilation et déploiement
├── target/             # (Ignoré par git) Dossier temporaire de compilation
│
├── core-api/           # 🧠 La librairie standard interne
│   ├── src/lib.rs      # Expose log(), db_read(), etc.
│   └── Cargo.toml
│
└── blocks/             # 🧱 Les Blocs Cognitifs (Plugins)
    ├── spy-plugin/     # Exemple : Plugin d'espionnage / Audit
    │   ├── src/lib.rs
    │   └── Cargo.toml  # Type 'cdylib' obligatoire
    │
    └── analyzer-consistency/
        └── ...

```

---

## 🚀 Workflow de Développement

### 1. Créer un nouveau bloc

Créez une nouvelle librairie dans le dossier `blocks/` :

```bash
cd src-wasm/blocks
cargo new --lib mon-algo

```

### 2. Configurer `Cargo.toml`

Modifiez `src-wasm/blocks/mon-algo/Cargo.toml` pour qu'il hérite du workspace et génère du WASM :

```toml
[package]
name = "mon-algo"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"] # ⚠️ INDISPENSABLE pour faire un .wasm

[dependencies]
genaptitude-core-api = { path = "../../core-api" }
serde = { workspace = true }
serde_json = { workspace = true }

```

### 3. Coder la logique (`lib.rs`)

Grâce à la `core-api`, le code est simple et lisible. Plus besoin de gérer les allocations mémoire manuellement.

```rust
use genaptitude_core_api as core;

#[no_mangle]
pub extern "C" fn run() -> i32 {
    // 1. Loguer quelque chose dans la console de GenAptitude
    core::log("🤖 Mon Algo : Démarrage de l'analyse...");

    // 2. Lire des données depuis la base de données de l'hôte
    // (Cette fonction appelle 'host_db_read' via le pont cognitif)
    let success = core::db_read("users", "admin");

    if success {
        core::log("✅ Donnée trouvée !");
        1 // Code retour succès
    } else {
        core::log("❌ Donnée introuvable.");
        0 // Code retour échec
    }
}

```

### 4. Compiler et Déployer

Ne lancez pas `cargo build` manuellement. Utilisez le script qui place automatiquement le résultat dans le "Magasin" (`wasm-modules/`) à la racine du projet.

Depuis la racine du projet (`~/genaptitude`) :

```bash
./src-wasm/build.sh

```

**Ce que fait le script :**

1. Il lance les tests unitaires (`cargo test`) pour chaque bloc.
2. Il compile en mode Release pour la cible `wasm32-unknown-unknown`.
3. Il copie le fichier `.wasm` final dans `wasm-modules/<nom-du-bloc>/`.

---

## 🔌 Capacités Disponibles (Core API)

Le plugin est isolé (sandbox), il ne peut rien faire d'autre que calculer, sauf s'il passe par ces fonctions offertes par `core-api` :

| Fonction                             | Description                                                                                                          |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| **`core::log(msg: &str)`**           | Envoie un message texte qui s'affichera dans les logs terminaux de GenAptitude.                                      |
| **`core::db_read(col, id) -> bool`** | Demande à GenAptitude de lire un document JSON dans la base locale. (Retourne `true` si l'appel technique a réussi). |

---

## ⚠️ Notes Techniques

- **Pas de `wasm-bindgen` JS** : Nous n'utilisons pas d'interface JavaScript. Le lien se fait directement entre Rust (Tauri) et Rust (Wasm).
- **Workspace** : Si vous ajoutez une dépendance commune (ex: `regex`), ajoutez-la dans le `Cargo.toml` racine (`[workspace.dependencies]`) pour éviter de la dupliquer.
- **Target** : Assurez-vous d'avoir la cible WASM installée : `rustup target add wasm32-unknown-unknown`.

```

```
