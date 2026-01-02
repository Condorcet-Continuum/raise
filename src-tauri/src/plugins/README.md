# 🧠 RAISE Cognitive Plugins Module

> **Architecture "Use-Case Factory" & Moteur WASM**

Ce module implémente le cœur de l'extensibilité de RAISE. Il permet de charger, sécuriser et exécuter des **"Blocs Cognitifs"** : des binaires WebAssembly (.wasm) capables d'interagir intelligemment avec les données de l'application via une API standardisée.

---

## 🏗️ Architecture Industrielle (Factory Pattern)

Le système ne se contente pas d'exécuter du WASM, il gère toute la chaîne de production logicielle via une séparation stricte :

1.  **L'Usine (`src-wasm/`)** :

    - C'est un **Workspace Rust autonome**.
    - Il contient le code source des plugins (`blocks/`) et l'API partagée (`core-api/`).
    - Il est invisible pour le projet Tauri principal (exclu du `Cargo.toml` racine).

2.  **La Chaîne de Montage (`src-wasm/build.sh`)** :

    - Script d'automatisation qui :
      - 🧪 Lance les tests unitaires de chaque bloc.
      - ⚙️ Compile le code en cible `wasm32-unknown-unknown`.
      - 📦 Copie et renomme les artefacts finaux.

3.  **Le Magasin (`wasm-modules/`)** :
    - Dossier de destination où sont stockés les fichiers `.wasm` compilés.
    - C'est ici que RAISE (le Host) vient piocher les plugins à charger.

---

## 🌉 Architecture Runtime (Host / Guest)

Une fois chargé dans l'application, le système repose sur une architecture isolée :

- **Host (RAISE / Tauri)** : Fournit le contexte, l'accès à la base de données (`JsonDb`), et injecte les capacités via le `Linker`.
- **Guest (Plugin / WASM)** : Contient la logique métier. Il ne peut interagir avec le monde extérieur que via la **RAISE Core API**.
- **Cognitive Bridge** : Le canal de communication mémoire partagée.

### Flux d'Exécution

1.  **Chargement** : `manager.rs` lit le fichier `.wasm` depuis `wasm-modules/`.
2.  **Instanciation** : `runtime.rs` crée un environnement `wasmtime` et lie les fonctions importées.
3.  **Bridge** : `cognitive.rs` injecte les fonctions système (`host_db_read`, `host_log`).
4.  **Exécution** : Le plugin exécute sa logique, appelle `core::db_read(...)`, et le Host traite la demande.

---

## 📂 Structure du Module Tauri (`src-tauri/src/plugins/`)

| Fichier            | Rôle & Responsabilité                                                                                                                |
| :----------------- | :----------------------------------------------------------------------------------------------------------------------------------- |
| **`mod.rs`**       | Point d'entrée du module.                                                                                                            |
| **`manager.rs`**   | **L'Orchestrateur**. Gère le stock des plugins chargés et déclenche leur exécution.                                                  |
| **`runtime.rs`**   | **Le Moteur**. Encapsule `wasmtime`. Configure le `Store` et gère le contexte mémoire.                                               |
| **`cognitive.rs`** | **Le Pont Cognitif**. Implémente les "Host Functions". Traduit les pointeurs mémoire du WASM en appels Rust natifs vers la `JsonDb`. |
| **`tests.rs`**     | Tests d'intégration validant le chargement et le sandboxing (génération de WASM à la volée).                                         |

---

## 👩‍💻 Guide du Développeur de Plugin

Pour créer un nouveau plugin, **ne modifiez pas `src-tauri`**. Travaillez uniquement dans l'usine `src-wasm`.

### 1. Création

Créez un nouveau dossier dans `src-wasm/blocks/` (ex: `mon-algo`).

### 2. Configuration (`Cargo.toml`)

Déclarez le type de librairie et la dépendance au Core :

```toml
[package]
name = "mon-algo"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"] # Indispensable pour générer du .wasm

[dependencies]
raise-core-api = { path = "../../core-api" }
serde = { workspace = true }
```

### 3. Code (`lib.rs`)

Utilisez l'API haut niveau (plus besoin de `unsafe`) :

```rust
use raise_core_api as core;

#[no_mangle]
pub extern "C" fn run() -> i32 {
    core::log("🚀 Démarrage de mon algo...");

    // Lecture sécurisée de la DB via le pont
    let success = core::db_read("users", "admin");

    if success {
         core::log("✅ Donnée trouvée !");
         1
    } else {
         0
    }
}

```

### 4. Compilation

Lancez simplement le script depuis la racine du projet :

```bash
./src-wasm/build.sh

```

Le fichier résultant sera disponible dans `wasm-modules/mon-algo/mon-algo.wasm`.

---

## 🔌 API du Pont Cognitif (Détails Techniques)

Sous le capot, `core-api` communique avec `cognitive.rs` via ces fonctions exportées par l'hôte :

| Fonction Host      | Signature (WASM)              | Description                                                                                                                  |
| ------------------ | ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| **`host_log`**     | `(ptr: i32, len: i32)`        | Affiche un message dans la console de logs de RAISE (`stdout`).                                                              |
| **`host_db_read`** | `(ptr: i32, len: i32) -> i32` | Reçoit une requête JSON `{col, id}`, interroge la DB, et logue le résultat (V1). Retourne `1` si l'appel technique a réussi. |

---

## 🔮 Roadmap / Améliorations Futures

1. **Communication Bidirectionnelle (Return Values)** : Implémenter l'allocation mémoire (`malloc`) dans le Guest pour que `host_db_read` puisse écrire le contenu JSON de la réponse directement dans la mémoire du plugin (actuellement, le Host affiche juste le résultat).
2. **Support WASI Complet** : Activer `filesystem_extended.rs` pour l'accès fichiers sécurisé.
3. **Hot-Reloading** : Rechargement à chaud des `.wasm` modifiés.

```

```

```

```
