# 📘 RAISE Utils - Façade Technique Unifiée

Ce module est la **colonne vertébrale technique** de l'application RAISE.
Il agit comme une **façade architecturale** pour isoler le code métier ("Core" et "CLI") des implémentations bas niveau et des librairies tierces (`std`, `tokio`, `serde`, `anyhow`, `reqwest`).

## ⚠️ Règles d'Or (The Golden Rules)

1. **Interdiction d'utiliser `std::fs**`: Tout accès fichier doit passer par`raise::utils::fs`.
2. **Interdiction d'utiliser `std::env**`: Toute configuration doit passer par`raise::utils::config`ou`raise::utils::env`.
3. **Interdiction d'utiliser `serde_json` directement** : Utilisez `raise::utils::json`.
4. **Pas d'erreurs `unwrap()` sauvages** : Utilisez les macros de gestion d'erreur et `AppError`.

---

## 📦 1. Primitives Standards (`mod.rs`)

Centralisation des types Rust essentiels pour éviter la pollution des imports `std`.

```rust
use raise::utils::{Arc, Future, Pin};

// Remplace : std::sync::Arc, std::future::Future, std::pin::Pin

```

---

## 📂 2. Système de Fichiers (`utils::fs`)

Gestion **asynchrone**, **atomique** et **instrumentée** (logs) des fichiers.

```rust
use raise::utils::fs::{self, Path, PathBuf};

// Lecture typée (Désérialisation auto)
let data: MyStruct = fs::read_json(&path).await?;

// Écriture Atomique (Crée .tmp, flush, et rename) -> Sécurité crash
fs::write_json_atomic(&path, &data).await?;

// Utilitaires
fs::ensure_dir(&path).await?;      // mkdir -p
fs::exists(&path).await;           // bool
fs::remove_file(&path).await?;     // safe delete

```

---

## ⚙️ 3. JSON & Sérialisation (`utils::json`)

Abstraction complète de `serde` et `serde_json`. Garantit un formatage cohérent et des erreurs typées `AppError`.

```rust
use raise::utils::json::{self, json, Value, Map, Serialize, Deserialize};

// Parsing
let obj: MyObj = json::parse(content_str)?;

// Conversion dynamique
let obj: MyObj = json::from_value(json_value)?;

// Stringify (Pretty Print par défaut dans RAISE)
let json_str = json::stringify_pretty(&obj)?;

// Fusion profonde (Deep Merge)
json::merge(&mut target_json, source_json);

```

---

## 🌍 4. Environnement (`utils::env`)

Accès typé et sécurisé aux variables d'environnement (`.env` ou Système).

```rust
use raise::utils::env;

// Récupération stricte (Erreur si manquant)
let api_key = env::get("API_KEY")?;

// Récupération optionnelle
let model = env::get_optional("MODEL_NAME"); // Option<String>

// Récupération avec défaut
let host = env::get_or("HOST", "localhost");

// Feature Flags (Supporte "true", "1", "yes", "on")
if env::is_enabled("DEBUG_MODE") { ... }

```

---

## 🚨 5. Gestion d'Erreurs (`utils::error`)

Système unifié. Distingue l'usage interne (bibliothèque) de l'usage externe (CLI/App).

```rust
use raise::utils::error::{AppError, Result, AnyResult, Context, anyhow};

// 1. Usage Interne (Bibliothèque / Core)
// Retourne toujours un AppError structuré
fn core_logic() -> Result<String> {
    if problem {
        return Err(AppError::NotFound("Item manquant".into()));
    }
    Ok("ok".into())
}

// 2. Usage Externe (CLI / Main)
// Flexible, permet d'utiliser le '?' sur n'importe quoi grâce à anyhow
fn main_handler() -> AnyResult<()> {
    core_logic().context("Le core a échoué")?;
    Ok(())
}

```

---

## 🛠️ 6. Configuration (`utils::config`)

Singleton global chargé au démarrage.

```rust
use raise::utils::config::AppConfig;

// Initialisation (au démarrage de l'app)
AppConfig::init()?;

// Accès partout dans le code
let cfg = AppConfig::get();
println!("DB Root: {:?}", cfg.database_root);

```

---

## 📢 7. Logging & Feedback (`utils::logger`)

Macros unifiées pour parler à l'utilisateur (Console) tout en loguant les détails techniques (Fichier `.log` + Tracing).

```rust
use raise::{user_info, user_success, user_error};

// Affiche "ℹ️ Traitement..." en console + Log structuré JSON avec module/ligne
user_info!("PROCESS_START", "Fichier: {}", filename);

// Affiche "✅ Succès..." en console
user_success!("DONE");

// Affiche "❌ Erreur..." en stderr
user_error!("FATAL_ERROR", "Code: {}", 500);

```

---

## 🗣️ 8. Internationalisation (`utils::i18n`)

Système de traduction léger.

```rust
use raise::utils::i18n;

// Initialisation
i18n::init_i18n("fr");

// Traduction
let msg = i18n::t("WELCOME_MESSAGE");

```

---

## 🌐 9. Réseau (`utils::net`)

Client HTTP unique, optimisé (Keep-Alive) et résilient.

```rust
use raise::utils::net;

// POST avec Retries exponentiels automatiques
let response: MyResponse = net::post_json_with_retry(
    "http://api.local/v1/chat",
    &request_body,
    3 // 3 tentatives max
).await?;

// GET simple
let text = net::get_simple("http://google.com").await?;

```
