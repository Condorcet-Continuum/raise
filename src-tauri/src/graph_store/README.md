### Fichier : `src-tauri/src/graph_store/README.md`

# 🧠 Graph Store (SurrealDB Embedded)

Ce module fournit une base de données locale, persistante et orientée graphe pour l'application. Il repose sur **SurrealDB** utilisé en mode embarqué (moteur `SurrealKv`).

## 🌟 Pourquoi SurrealDB ? (Fonctionnalités Clés)

Contrairement à une base de données traditionnelle (comme SQLite) ou purement documentaire (comme MongoDB), SurrealDB est **multi-modèle**. Ce module exploite trois capacités majeures :

### 1. Modèle Hybride : Document + Graphe

SurrealDB permet de stocker des objets JSON complexes (Documents) tout en les reliant directement entre eux (Graphe).

- **Nœuds (Nodes)** : Ce sont des enregistrements classiques (ex: `person:alice`, `file:report_pdf`). Ils contiennent des données JSON arbitraires.
- **Arêtes (Edges)** : Ce sont des liens directionnels qui possèdent eux-mêmes des données.
  - _Exemple_ : `alice` -> `working_on { "since": "2023" }` -> `project_raise`.
  - Cela permet de requêter des relations complexes sans faire de `JOIN` coûteux comme en SQL.

### 2. Recherche Vectorielle (IA / RAG)

Ce module utilise la capacité native de SurrealDB à stocker des vecteurs (`Vec<f32>`) et à calculer des distances mathématiques.

- **Fonctionnalité** : Recherche sémantique ("Trouver les documents qui parlent de concepts similaires à X").
- **Méthode** : Similarité Cosinus (`vector::similarity::cosine`).
- **Usage** : Idéal pour implémenter du RAG (Retrieval-Augmented Generation) localement.

### 3. Moteur Embarqué (Embedded)

L'application n'a pas besoin de lancer un serveur Docker ou un processus séparé.

- La base de données est un simple dossier/fichier (`raise_graph.db`) géré directement par le binaire Rust via `SurrealKv` (basé sur RocksDB).
- **Avantage** : Latence zéro (pas de réseau) et déploiement simplifié.

---

## 🛠 Architecture Technique

### Le défi de la Sérialisation

Un point critique de ce module est la gestion des types. SurrealDB utilise des types binaires internes riches (ex: `Thing` pour les IDs `table:id`, `Datetime`, etc.) qui ne sont pas compatibles nativement avec le format JSON standard.

**La solution implémentée (`surreal_impl.rs`) :**
Le client agit comme un "pont" de traduction.

1.  **Entrée** : Il accepte du JSON standard (`serde_json::Value`).
2.  **Traitement** : Il utilise les méthodes natives (`.create`, `.select`) ou du SQL avec transtypage (`<string>id`) pour interagir avec le moteur.
3.  **Sortie** : Il convertit les structures binaires (`surrealdb::sql::Object`) en JSON propre avant de les renvoyer à l'application.

---

## 🚀 Guide d'Utilisation

### 1. Initialisation

Démarre le moteur embarqué et prépare le namespace/database.

```rust
use crate::graph_store::surreal_impl::SurrealClient;
use std::path::PathBuf;

let data_dir = PathBuf::from("./data");
let client = SurrealClient::init(data_dir).await?;

```

### 2. Gestion des Nœuds (Upsert)

La méthode `upsert_node` est idempotente : elle crée le nœud s'il n'existe pas, ou le met à jour s'il existe déjà.

```rust
use serde_json::json;

// Table: "task", ID: "t1"
client.upsert_node("task", "t1", json!({
    "title": "Finir le README",
    "status": "todo",
    "tags": ["docs", "rust"]
})).await?;

```

### 3. Création de Relations (Graphe)

Crée un lien sémantique entre deux entités. La syntaxe est `DE -> RELATION -> VERS`.

```rust
// Lie la tâche 't1' à l'utilisateur 'alice'
client.create_edge(
    ("person", "alice"), // Source
    "assigned_to",       // Nom de la relation
    ("task", "t1")       // Destination
).await?;

```

### 4. Recherche de Similarité (Vecteurs)

Récupère les objets les plus proches mathématiquement d'un vecteur donné.

```rust
let embedding = vec![0.12, 0.88, 0.04, ...]; // Vecteur généré par un modèle IA
let limit = 10;

// Cherche dans la table 'chunk'
let results = client.search_similar("chunk", embedding, limit).await?;

```

---

## ⚠️ Pièges Courants (Troubleshooting)

### Erreur : `Serialization error: expected enum variant...`

Cette erreur survient si vous essayez de récupérer le résultat brut d'une requête SQL via `take::<Value>()` sans précautions.

- **Cause** : Le moteur renvoie une Structure binaire, mais `serde_json::Value` attend un Enum.
- **Solution** : Utilisez toujours les méthodes wrapper de `SurrealClient` (`select`, `upsert_node`) qui gèrent la conversion `Object -> JSON` en interne.

### Erreur : `Parse error` sur les IDs

SurrealDB force le format `table:id`.

- ❌ `id: "123"` (Invalide sans table)
- ✅ `id: "user:123"` (Valide)
- Le module gère cela en demandant `table` et `id` séparément dans les arguments des fonctions.

---

## 🧪 Tests

Les tests unitaires couvrent le cycle de vie complet (CRUD, Relations, Vecteurs) et valident la correction des conversions de types.

```bash
cargo test graph_store

```
