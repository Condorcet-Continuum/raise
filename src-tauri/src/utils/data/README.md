# 📦 RAISE Foundation - Module `data` (Modélisation, JSON & Configuration)

Bienvenue dans le centre de traitement de l'information de l'architecture RAISE (`src-tauri/src/utils/data`). 

Ce module a une mission claire : **bannir l'ambiguïté des types natifs**. Il encapsule la machinerie de sérialisation (`serde`), la manipulation du JSON (`serde_json`), et la configuration globale de l'application, en fournissant des interfaces strictement sémantiques et "AI-Ready".



---

## 🛑 DIRECTIVES SYSTÈMES STRICTES (POUR LES AGENTS IA)

1. **Bannissement des Types Natifs Ambigus** : Vous ne devez **JAMAIS** utiliser `std::collections::HashMap` ou `BTreeMap` directement dans les modèles de données. Utilisez **EXCLUSIVEMENT** les alias sémantiques exportés par la façade : `UnorderedMap`, `OrderedMap`, et `UniqueSet`.
2. **Bannissement de Serde Brut** : N'importez jamais `serde::{Serialize, Deserialize}` ou `serde_json::Value` directement. Utilisez `Serializable`, `Deserializable`, `JsonValue`, et `JsonObject`.
3. **Parsing Sécurisé** : L'appel direct à `serde_json::from_str` ou `to_string` est **interdit**. Vous devez utiliser les fonctions de `crate::utils::data::json` qui garantissent la remontée d'erreurs structurées (`RaiseResult`).

---

## 1. 🏷️ Typage Sémantique (La Loi de l'Intention)

Le fichier `mod.rs` agit comme un dictionnaire universel pour le framework. En forçant l'utilisation de ces alias, l'IA (et le développeur) comprend immédiatement l'intention derrière une structure de données.

* **Collections** : `UnorderedMap` (performance O(1), ordre non garanti), `OrderedMap` (trié alphabétiquement), `UniqueSet` (déduplication).
* **Sérialisation** : Les traits `Serializable` et `Deserializable` remplacent les macros standards pour standardiser la tuyauterie.
* **Manipulation JSON** : 
  * `JsonValue` : Représente n'importe quelle donnée JSON valide.
  * `JsonObject` : Représente strictement un dictionnaire `{ "clé": "valeur" }`.
  * `json_value!({ ... })` : La macro pour construire du JSON dynamiquement.

---

## 2. 🧩 L'Écosystème JSON AI-Ready (`json.rs`)

Le parsing JSON est l'une des sources de crash les plus fréquentes. Les wrappers fournis par `json.rs` transforment ces crashs silencieux en erreurs explicites, prêtes à être analysées par une IA.

### Les Wrappers de Désérialisation
* **`deserialize_from_str<T>(...)`** : Désérialise une chaîne. **L'astuce "AI-Ready"** : En cas d'échec (ex: virgule manquante), la fonction capture automatiquement les 100 premiers caractères du JSON incriminé (`snippet`) et les injecte dans le contexte de l'erreur `ERR_JSON_PARSE`.
* **`deserialize_from_value<T>(...)`** : Convertit un `JsonValue` dynamique en structure typée Rust `T`.
* **`deserialize_from_bytes<T>(...)`** : Lit directement depuis un buffer mémoire.

### Les Wrappers de Sérialisation
* **`serialize_to_string(...)`** : Format compact (réseau, BDD).
* **`serialize_to_string_pretty(...)`** : Format indenté (logs humains, debug).
* **`serialize_to_value(...)`** : Transforme une structure Rust en `JsonValue` manipulable dynamiquement.

---

## 3. ⚙️ Le Cerveau de Configuration (`config.rs`)

L'objet `AppConfig` est le Singleton (`StaticCell`) qui détient la vérité absolue sur l'environnement d'exécution de l'application (chemins, bases de données actives, variables d'IA).

### Architecture en Scopes (Contextes)
La configuration fusionne intelligemment plusieurs niveaux :
1. **Niveau Système (Global)** : Paramètres de l'application (DApp active, services, domaine système).
2. **Niveau Machine (`workstation`)** : Surcharge spécifique à la machine physique (ex: `hostname`).
3. **Niveau Utilisateur (`user`)** : Préférences liées à l'utilisateur système actif (langue, domaine par défaut).

### Règle d'Accès
* **Initialisation** : Appelée une seule fois au démarrage de l'app via `AppConfig::init()`.
* **Lecture** : Dans n'importe quel service métier, obtenez la configuration immuable en appelant `AppConfig::get()`.
  * *Exemple* : `AppConfig::get().get_path("PATH_RAISE_DOMAIN")`.

### Fallbacks et Robustesse
Pour éviter que l'application ne crashe si un fichier JSON est incomplet, toutes les structures de configuration utilisent des fonctions de repli (`#[serde(default = "fallback_...")]`) garantissant des valeurs par défaut saines (ex: tableau de composants vide plutôt que valeur `null`).

```

