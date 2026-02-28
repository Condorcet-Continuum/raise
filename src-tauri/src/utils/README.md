# 📚 Référentiel Raise Foundation (v1.2)

Ce document définit la nomenclature réelle, les constantes et les directives de développement du socle technique (`src-tauri/src/utils`).

## 1. Le Type de Retour Unique : `RaiseResult<T>`

Pour lever toute ambiguïté avec le `Result` standard de Rust ou celui de bibliothèques tierces, toutes les fonctions de la fondation retournent un `RaiseResult`.

* **Définition** : `pub type RaiseResult<T> = std::result::Result<T, AppError>;`.
* **Objectif** : Garantir que l'erreur retournée est toujours une `AppError` sérialisable et contextualisée.

---

## 2. Nomenclature des Fonctions (Noms Réels)

L'utilisation de `std::fs` ou `tokio::fs` directement dans les couches supérieures est interdite. Utilisez les fonctions réelles suivantes :

### A. I/O Physiques & Sécurité (`io::`)

Les fonctions I/O sont asynchrones par défaut et intègrent des vérifications de sécurité.

| Fonction Réelle | Description | Source |
| --- | --- | --- |
| `async_path_exists(path)` | Vérifie l'existence d'un fichier/dossier sans paniquer. | `fs.rs` |
| `async_file_read_json<T>(path)` | Lit et désérialise un JSON avec diagnostic `NotFound` précis. | `fs.rs` |
| `async_file_write_atomic(path, data)` | Écrit des octets via un fichier `.tmp` pour éviter la corruption. | `fs.rs` |
| `async_file_write_json_atomic(path, data)` | Sérialise et écrit un objet JSON de manière atomique. | `fs.rs` |
| `async_dir_create_all(path)` | Crée récursivement un répertoire (équivalent `mkdir -p`). | `fs.rs` |
| `async_write_safe(rel_path, data)` | Écrit uniquement à l'intérieur du `ProjectScope` (Sandbox). | `fs.rs` |

### B. Manipulation de Données (`data::`)

Manipulation mémoire du JSON et formats binaires.

| Fonction Réelle | Description | Source |
| --- | --- | --- |
| `json_parse<T>(str)` | Transforme une chaîne en structure typée. | `json.rs` |
| `json_serialize_pretty(data)` | Sérialise en JSON lisible (standard pour les fichiers de config). | `json.rs` |
| `json_serialize_compact(data)` | Sérialise en JSON condensé (standard pour le réseau/stockage). | `json.rs` |
| `json_deep_merge(a, b)` | Fusion récursive de deux `serde_json::Value`. | `json.rs` |
| `bin_serialize<T>(data)` | Sérialise en format binaire compact via Bincode. | `json.rs` |

### C. Système & Exécution (`sys::`)

Interaction avec l'OS de manière non-bloquante.

| Fonction Réelle | Description | Source |
| --- | --- | --- |
| `sys_exec_wait(cmd, args)` | Lance une commande et attend la fin pour capturer stdout/stderr. | `os.rs` |
| `sys_pipe_to_tool(cmd, input)` | Envoie une chaîne dans le stdin d'un outil (ex: `rustfmt`). | `os.rs` |

---

## 3. Constantes Système (SSoT)

Les constantes sont définies dans `config.rs` et ne doivent jamais être écrites en dur.

* **`SYSTEM_DOMAIN`** : Nom du domaine racine (`_system`).
* **`SYSTEM_DB`** : Nom de la base de données centrale (`_system`).
* **`PATH_RAISE_DOMAIN`** : Clé de configuration pour le stockage physique principal.
* **`PATH_LOGS`** : Emplacement des journaux d'audit et de debug.

---

## 4. Directives et Exceptions

### Gestion des Erreurs Bas Niveau

Chaque erreur doit être porteuse d'un contexte pour permettre à l'IA de s'auto-corriger.

 
### Directives pour les Agents IA

1. **Usage du Prelude** : Tout module doit commencer par `use crate::utils::prelude::*;` pour accéder aux types `RaiseResult`, `AppError`, et aux fonctions `async_`.
2. **Atomicité par défaut** : Toute écriture de fichier JSON **doit** passer par `async_file_write_json_atomic`.
3. **Pas de chemins relatifs "purs"** : Utilisez toujours `AppConfig::get().get_path("...")` pour construire des chemins absolus sécurisés.

---

## 5. Plan de Migration des Fichiers

| Fichier | Statut | Action à entreprendre |
| --- | --- | --- |
| `error.rs` | 🛠️ En cours | Renommer `Result` ➔ `RaiseResult`. |
| `fs.rs` | 🛠️ En cours | Implémenter les fonctions réelles `async_*`. |
| `json.rs` | 🛠️ En cours | Renommer `parse` ➔ `json_parse`, etc. |
| `file_storage.rs` | ⏳ Attente | Remplacer les appels `io::read_json` par `io::async_file_read_json`. |

---
 