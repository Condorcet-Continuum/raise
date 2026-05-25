# 📚 Référentiel RAISE Foundation (v2.1 - Architecture Isolée & Sémantique)

Ce document définit la constitution technique, la nomenclature et les protocoles de développement du socle `src-tauri/src/utils`. L'architecture RAISE repose sur un principe de **Souveraineté de la Fondation** : aucun module métier ne doit dépendre d'une bibliothèque tierce. Tout passe par nos façades.

---

## 🏛️ 1. Structure des Sous-Domaines (La Forteresse)

Le module `utils` est segmenté en six forteresses autonomes. Chaque domaine possède son propre `README.md` détaillé.

| Domaine | Responsabilité Critique |
| --- | --- |
| **`core/`** | **Le Cœur** : Gestion des erreurs `RaiseResult`, concurrence (`SharedRef`), attributs `#[async_interface]`, `#[async_test]` et macros système. |
| **`data/`** | **La Matière** : Manipulation JSON AI-Ready, alias de collections (`UnorderedMap`), et le singleton `AppConfig`. |
| **`io/`** | **Le Physique** : Système de fichiers asynchrone, écriture atomique, gestion des chemins (`PathBuf`) et accès OS. |
| **`net/`** | **Les Flux** : Client HTTP avec retry, serveur local REST, et nœuds P2P décentralisés. |
| **`context/`** | **L'Esprit** : Gestion des sessions utilisateurs, internationalisation (i18n) en RAM, et logging structuré JSONL. |
| **`testing/`** | **Le Miroir** : Sandboxes isolées (`AgentDbSandbox`) et mocks pour garantir des tests sans effets de bord. |

---

## 🎭 2. Le Prélude : L'Unique Point d'Entrée

L'importation de `serde`, `tokio`, `anyhow`, `reqwest` ou `libp2p` est **formellement interdite** dans les services métiers.

👉 **Règle d'Or** : Tout fichier métier (Services, Blockchain, UI-Commands) doit utiliser exclusivement :

```rust
use crate::utils::prelude::*;

```

Le prélude expose les alias sémantiques (ex: `SharedRef` au lieu de `Arc`) pour masquer la complexité technique derrière une intention métier.

---

## 🚨 3. Le Système d'Erreur "AI-Ready"

Toutes les fonctions de la fondation retournent un **`RaiseResult<T>`**.

* **`AppError::Structured`** : Capture automatiquement le `service`, le `subdomain`, le `component` et l' `action` via les macros `build_error!` ou `raise_error!`.
* **Observabilité** : En cas d'échec de parsing JSON, le système capture un `snippet` du texte malformé pour faciliter le diagnostic automatique par l'IA.

---

## 🛠️ 4. Nomenclature des Façades (API Publique)

### A. I/O & Système de Fichiers (`fs::`)

| Fonction | Usage |
| --- | --- |
| `exists_async(path)` | Vérification non-bloquante de l'existence. |
| `write_json_atomic_async(path, data)` | Sérialisation et écriture sécurisée (anti-corruption). |
| `read_json_async<T>(path)` | Lecture avec typage fort et diagnostic d'erreur intégré. |

### B. Données & JSON (`json::`)

| Fonction | Usage |
| --- | --- |
| `deserialize_from_str<T>(s)` | Parsing ultra-sécurisé avec contexte d'erreur. |
| `serialize_to_string_pretty(v)` | Génération de JSON lisible pour la configuration. |
| `deep_merge_values(a, b)` | Fusion récursive de deux objets `JsonValue`. |

### C. Réseau & Connectivité (`net::`)

| Fonction | Usage |
| --- | --- |
| `get_client()` | Récupère le singleton `HttpClient` (pool de connexions). |
| `post_authenticated_async(...)` | Requête sécurisée avec retry exponentiel automatique. |
| `build_p2p_node_async(...)` | Initialisation d'un nœud P2P sécurisé (libp2p). |

---

## 🧩 5. Dictionnaire des Macros & Attributs

Pour l'Agent IA, ces macros sont les primitives de base du langage RAISE :

* **`#[async_interface]`** : À placer sur tout `trait` contenant des méthodes `async`.
* **`#[async_test]`** : À placer sur toute fonction de test `async`.
* **`raise_error!(code, error, context)`** : Interrompt l'exécution proprement (Divergence).
* **`user_info!` / `user_error!**` : Notifie l'utilisateur via l'UI et logue l'événement en JSONL.

---

## 📈 6. État de la Migration (V2.1)

| Phase | Statut | Action Requise |
| --- | --- | --- |
| **Isolation `utils**` | ✅ 100% | Les sous-domaines sont hermétiques et auto-documentés. |
| **Zéro Prelude Interne** | ✅ 100% | Les fichiers de `utils` n'utilisent plus le prélude (évite les cycles). |
| **Services Métiers** | 🛠️ 40% | En cours de nettoyage pour supprimer les imports `serde`/`tokio` directs. |
| **Bannissement `anyhow**` | 🛠️ 20% | Remplacer `AnyResult` par `RaiseResult` dans les commandes Tauri. |

---

**Directives finales pour l'IA** : Si vous devez ajouter une dépendance, ajoutez-la d'abord dans une façade de `utils`, créez un alias sémantique, et exposez-le via le prélude. **Ne polluez jamais le domaine métier.**.

---
 