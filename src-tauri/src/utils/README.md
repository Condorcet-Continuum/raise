# 📜 Charte de Nomenclature Raise

Cette charte définit le langage universel pour l'accès aux ressources du système. **Toute fonction exportée dans `utils/mod.rs` doit suivre cette taxonomie.**

## 1. Structure de Nomination

Le nom d'une fonction doit répondre à quatre questions : *Comment ? Quoi ? Sur quoi ? Avec quelle garantie ?*

**Syntaxe :** `[MODE]_[ACTION]_[FORMAT]_[SECURITE]`

### A. Préfixes de Mode (L'exécution)

* `async_` : Opérations non-bloquantes utilisant `tokio`. C'est le standard pour les E/S.
* `sync_` : Opérations bloquantes (CPU-bound ou legacy). À utiliser avec parcimonie.

### B. Verbes d'Action (L'intention)

* `read` : Récupération de données depuis le disque.
* `write` : Persistance de données sur le disque.
* `sys` : Interaction avec le système d'exploitation.
* `net` : Communication réseau.
* `json` : Manipulation de structures de données en mémoire.

### C. Suffixes de Sécurité (La garantie)

* `_atomic` : Garantit que l'écriture est totale ou nulle (via un fichier `.tmp` puis renommage).
* `_safe` : L'opération est confinée dans le `ProjectScope` et ne peut pas accéder au reste du disque.
* `_compressed` : Utilise l'algorithme Zstd pour réduire l'empreinte disque.

---

## 2. Table de Référence des Fonctions

### I/O & Système de Fichiers (`io::`)

| Ancien Nom (Technique) | Nouveau Nom (Raise) | Source de l'implémentation |
| --- | --- | --- |
| `fs::read_to_string` | `async_read_str` | Tokio FS |
| `fs::read_json` | `async_read_json` | Utils FS |
| `fs::write_atomic` | `async_write_atomic` | Utils FS |
| `fs::write_json_atomic` | `async_write_json_atomic` | Utils FS |
| `fs::read_json_compressed` | `async_read_json_compressed` | Compression + FS |
| `fs::ProjectScope::write` | `async_write_safe` | Sécurité Sandboxing |

### Data & Transformation (`data::`)

| Ancien Nom (Technique) | Nouveau Nom (Raise) | Source de l'implémentation |
| --- | --- | --- |
| `json::parse` | `json_parse` | Serde Wrapper |
| `json::stringify` | `json_serialize_compact` | Serde Wrapper |
| `json::stringify_pretty` | `json_serialize_pretty` | Serde Wrapper |
| `json::merge` | `json_deep_merge` | Logic de fusion récursive |
| `json::to_binary` | `bin_serialize` | Bincode Wrapper |

### Système & Réseau (`sys::` / `net::`)

| Ancien Nom (Technique) | Nouveau Nom (Raise) | Source de l'implémentation |
| --- | --- | --- |
| `os::exec_command` | `sys_exec_wait` | Processus standard |
| `os::pipe_through` | `sys_pipe_to_tool` | Stdin/Stdout redirection |
| `net::post_authenticated` | `net_post_retry` | Client HTTP + Auth |

---

## 3. Référentiel des Constantes Système

Les constantes ne doivent jamais être écrites en dur ("hardcoded"). Elles proviennent exclusivement de `config.rs`.

* `SYSTEM_DOMAIN` : Le domaine racine de l'application (`_system`).
* `SYSTEM_DB` : Le nom de la base de données de configuration.
* `PATH_RAISE_DOMAIN` : Chemin physique vers le stockage racine.
* `PATH_LOGS` : Emplacement des journaux d'événements.

---

## 4. Instructions pour les Agents IA

> ⚠️ **Règle d'or :** L'importation directe de `std::fs` ou `tokio::fs` est interdite dans les modules de haut niveau (Agents, Commands).
> L'Agent **DOIT** utiliser le `prelude` ou les façades renommées dans `crate::utils`.

**Exemple de transformation attendue :**

* *Mauvais code IA :* `tokio::fs::write("config.json", serde_json::to_string(&cfg)?).await?`
* *Code Raise :* `utils::io::async_write_json_atomic("config.json", &cfg).await?`

---

### Prochaine étape suggérée

 