# 🛠️ Module Utils (Fondations)

Ce module fournit les briques techniques transversales nécessaires au fonctionnement du backend Rust.
Il est conçu pour être **thread-safe**, **performant** et **facilement utilisable** par les autres modules (AI, JsonDB, Commands).

## 📂 Structure

| Fichier         | Rôle                                                                      |
| :-------------- | :------------------------------------------------------------------------ |
| **`config.rs`** | Gestion de la configuration globale via un Singleton (env vars, chemins). |
| **`error.rs`**  | Gestion centralisée des erreurs avec sérialisation pour le Frontend.      |
| **`logger.rs`** | Configuration du système de logs structurés (`tracing`).                  |
| **`mod.rs`**    | Point d'entrée et re-exports.                                             |

---

## ⚙️ Configuration (`config.rs`)

La configuration est chargée une seule fois au démarrage (dans `main.rs`) via `dotenvy` et stockée dans un `OnceLock` (Singleton). Elle est ensuite accessible partout dans le code sans avoir à passer l'objet en paramètre.

### Variables d'Environnement (.env)

Le système priorise les variables définies dans le fichier `.env` ou l'environnement système.

| Variable            | Description                                 | Valeur par défaut       |
| :------------------ | :------------------------------------------ | :---------------------- |
| `APP_ENV`           | Environnement d'exécution (`dev`, `prod`)   | `"development"`         |
| `PATH_RAISE_DOMAIN` | Dossier racine pour la base de données JSON | `~/raise_domain`        |
| `RAISE_LOCAL_URL`   | URL du LLM local (Llama.cpp)                | `http://localhost:8081` |
| `RAISE_GEMINI_KEY`  | Clé API pour le mode Cloud (Optionnel)      | `None`                  |

### Utilisation dans le code

```rust
use crate::utils::AppConfig;

fn ma_fonction() {
    // Accès thread-safe et instantané (lecture mémoire)
    let config = AppConfig::get();

    println!("Mode : {}", config.env_mode);
    println!("DB Path : {:?}", config.database_root);
}
```

> **Note :** Si `AppConfig::init()` n'a pas été appelé au début du `main`, l'appel à `get()` provoquera un panic pour éviter des comportements indéfinis.

---

## 🚨 Gestion des Erreurs (`error.rs`)

Nous utilisons le pattern `AppError` qui unifie toutes les erreurs possibles (IO, Parsing, Réseau) en un seul type.

### Caractéristiques

1.  **Interopérabilité Frontend** : L'énumération implémente manuellement `Serialize`. Lorsqu'une commande Tauri renvoie une `AppError`, elle est automatiquement convertie en chaîne de caractères (`String`) pour être affichée proprement dans l'UI React (via `console.error` ou un Toast).
2.  **Conversion Automatique** : Grâce à `thiserror` et `From<T>`, les erreurs standards (`std::io::Error`, `serde_json::Error`) sont converties implicitement avec `?`.

### Exemple

```rust
use crate::utils::Result; // Alias vers Result<T, AppError>

fn lire_fichier() -> Result<String> {
    // L'erreur IO est convertie automatiquement en AppError::Io
    let content = std::fs::read_to_string("inconnu.txt")?;
    Ok(content)
}
```

---

## 🪵 Logging (`logger.rs`)

Le système de log repose sur la crate **`tracing`**. Il offre des logs structurés, asynchrones et colorés.

### Niveaux de Log

Le niveau de verbosité est contrôlé par la variable `RUST_LOG`.

```bash
# Voir uniquement les infos importantes
RUST_LOG=info cargo run

# Voir tout ce qui se passe dans RAISE (très verbeux)
RUST_LOG=raise=debug cargo run

# Cibler un module spécifique
RUST_LOG=raise::json_db=trace cargo run
```

### Utilisation

```rust
// Au lieu de println!
tracing::info!("Serveur démarré");
tracing::warn!("Fichier de config absent, utilisation des défauts");
tracing::error!("Échec critique de la base de données : {}", e);
```

```

```
