# ⚙️ RAISE Foundation - Module `core` (Fondations, Erreurs & Concurrence)

Bienvenue dans le noyau absolu de l'architecture RAISE (`src-tauri/src/utils/core`).

Ce module est la fondation de plus bas niveau du framework. Il est totalement isolé et ne dépend d'aucun autre module métier (à l'exception des types de données purs dans `utils::data`). Il fournit la machinerie lourde : gestion des erreurs structurées, macros d'observabilité, concurrence (locks) et accès au runtime asynchrone.

---

## 🛑 DIRECTIVES SYSTÈMES STRICTES (POUR LES AGENTS IA)

1. **Interdiction Totale des Primitives Standard** : Vous ne devez **JAMAIS** utiliser `std::sync::Arc`, `std::sync::Mutex`, ou `tokio::sync::RwLock` directement dans le code métier. Utilisez **EXCLUSIVEMENT** les alias sémantiques fournis par ce module (ex: `SharedRef`, `SyncMutex`, `AsyncRwLock`).
2. **Le Contrat `RaiseResult`** : Toute fonction métier pouvant échouer **DOIT** retourner un `RaiseResult<T>`. N'utilisez pas `Result<T, E>` avec des erreurs string ou `anyhow` pour la logique de domaine.
3. **Bannissement de `tracing` Brut** : Les appels à `tracing::info!` ou `tracing::error!` sont strictement interdits en dehors de ce module. Vous devez utiliser les macros métier (ex: `user_info!`, `user_error!`).
4. **Zéro `unwrap()`** : La panique est inacceptable. Gérez les erreurs, ou propagez-les avec `?` ou `raise_error!`.

---

## 1. 🚨 Gestion des Erreurs "AI-Ready" (`error.rs`)

L'erreur dans RAISE n'est pas un simple texte, c'est un **objet de données structuré** conçu pour le diagnostic automatisé par l'IA.

### L'objet `AppError::Structured`

Toute erreur générée par le système est encapsulée dans `AppError::Structured(Box<StructuredData>)`. Elle contient :

- `service`, `subdomain`, `component`, `action` : Localisation exacte du crash (générée automatiquement par les macros).
- `code` : La clé d'erreur (ex: `"ERR_DB_READ"`).
- `message` : Le message traduit via `i18n` pour l'utilisateur.
- `context` : Un objet JSON contenant toutes les variables d'exécution au moment du crash.

### 🛡️ Le Contrat Frontend (Sécurité)

Lorsqu'un `AppError` est renvoyé à l'interface Tauri (Frontend), notre implémentation du trait `Serializable` **filtre les données sensibles**. Le frontend ne reçoit **que le message texte traduit**. Tout le contexte technique et les stacktraces restent confinés dans les logs sécurisés du backend.

---

## 2. 📡 Macros d'Observabilité (`macros.rs`)

Ces macros sont le seul moyen autorisé d'interagir avec le logger et le système d'erreurs. Elles injectent automatiquement la localisation du code, le contexte JSON, et traduisent les messages.

### Signaler une information ou un événement

- `user_info!("MSG_START", json_value!({"port": 8080}))` : Logue une info.
- `user_warn!("MSG_RETRY")` : Logue un avertissement.
- `user_success!("MSG_DONE")` : Logue un succès.

### Déclencher une erreur structurée

- **`raise_error!(code, error = technique, context = json)`** : Macro de divergence absolue. Elle construit l'erreur structurée et exécute immédiatement un `return Err(...)`. Ne peut être utilisée que dans une fonction retournant `RaiseResult`.
- **`build_error!(...)`** : Idem, mais retourne l'objet `AppError` au lieu de faire un `return` (utile pour les conversions `map_err`).

_Exemple :_

```rust
raise_error!(
    "ERR_FILE_NOT_FOUND",
    error = os_error,
    context = json_value!({ "path": filepath, "attempt": 3 })
);

```

---

## 3. 🚦 Concurrence et Runtime Sémantique (`mod.rs`)

Pour garantir que l'IA comprend _l'intention_ derrière une allocation mémoire ou un thread, les types génériques de Rust et Tokio sont masqués derrière une nomenclature stricte :

### Partage & Verrous

- `SharedRef<T>` (alias de `Arc<T>`) : Partage de propriété immuable entre threads.
- `AsyncRwLock<T>` / `AsyncMutex<T>` : Verrous pour le code asynchrone (Tokio).
- `SyncRwLock<T>` / `SyncMutex<T>` : Verrous rapides pour le code synchrone.
- `StaticCell<T>` (alias de `OnceLock<T>`) : Pour les singletons et l'état global.

### Runtime (Tokio)

- `spawn_cpu_task` : Délègue une tâche lourde (ex: chiffrement) au pool CPU pour ne pas geler l'asynchronisme.
- `spawn_async_task` : Lance une tâche non bloquante en arrière-plan.

### Temps et Identifiants

- `UniqueId` : Génération d'UUID v4.
- `UtcClock` / `UtcTimestamp` : Manipulation du temps absolue (toujours privilégier UTC pour le stockage).

 
## 🧩 Attributs de Langage (Extensions Async)

Le framework RAISE standardise l'asynchronisme via deux attributs majeurs exposés dans le `core` :

1. **`#[async_interface]`** : Remplace `#[async_trait]`. Doit être utilisé sur tout `trait` définissant des méthodes `async` pour garantir la compatibilité avec le moteur d'exécution.
2. **`#[async_test]`** : Remplace `#[tokio::test]`. Doit être utilisé pour marquer les fonctions de test qui nécessitent un `await`.

**Règle d'or :** L'utilisation directe des crates `tokio` ou `async_trait` dans les fichiers `services`, `blockchain` ou `network` est une violation architecturale.


```
