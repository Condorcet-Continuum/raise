# Module json_db

> **Version :** 1.3
> **Mise à jour :** Décembre 2025
> **Type :** Moteur SGBD NoSQL Embarqué, Transactionnel & Sémantique.

---

## 📦 Vue d'Ensemble

Le module **json_db** est le cœur de persistance de la plateforme GenAptitude. Il s'agit d'une base de données orientée documents (JSON) qui hybride les caractéristiques d'un **SGBD NoSQL** classique avec celles d'un **Graphe de Connaissances (Knowledge Graph)** via JSON-LD.

Elle est conçue pour être :

1.  **Souveraine** : Les données résident dans des fichiers standards (`.json`) lisibles par l'humain.
2.  **Robuste** : Les transactions ACID garantissent l'intégrité via un journal (WAL).
3.  **Intelligente** : Elle comprend la sémantique des données (Ontologie Arcadia) et offre un langage de requête SQL.

---

## 🏗️ Architecture Technique

### 1\. Organisation Physique

Les données sont stockées selon la hiérarchie définie par la variable d'environnement `PATH_GENAPTITUDE_DOMAIN`.

```text
<domain_root>/
├── <space>/                  # Espace de travail (ex: "un2")
│   ├── <database>/           # Base de données (ex: "_system")
│   │   ├── _system.json      # Index Système (Catalogue des collections)
│   │   ├── _wal/             # Write-Ahead Log (Journal des transactions)
│   │   ├── schemas/v1/       # Registre des schémas JSON (Structure)
│   │   └── collections/
│   │       └── <collection>/ # (ex: "actors")
│   │           ├── _meta.json        # Configuration & Index définis
│   │           ├── _indexes/         # Index Binaires (.idx) Hash/BTree/Text
│   │           ├── <uuid>.json       # Documents (JSON-LD compact)
│   │           └── ...
```

### 2\. La Stack Logicielle

Le moteur est divisé en couches de responsabilité distinctes :

| Couche          | Module                 | Rôle Principal                                                                                      |
| :-------------- | :--------------------- | :-------------------------------------------------------------------------------------------------- |
| **Interface**   | `collections::manager` | Point d'entrée CRUD. Orchestre la validation, la sémantique et la persistance.                      |
| **Transaction** | `transactions`         | Garantit l'atomicité (ACID). Gère le verrouillage (`LockManager`) et le WAL.                        |
| **Sémantique**  | `jsonld`               | **Nouveau**. Enrichit les données (`@context`), valide les types (`@type`) et gère l'expansion RDF. |
| **Requête**     | `query`                | **Nouveau**. Moteur SQL (`SELECT`, `WHERE`, `ORDER BY`), Parseur et Exécuteur avec Projections.     |
| **Indexation**  | `indexes`              | Maintient des structures de recherche rapides (Hash, BTree) synchronisées avec les données.         |
| **Stockage**    | `storage`              | Gestion bas niveau des fichiers, I/O atomiques et cache.                                            |

---

## 🧠 Couche Sémantique & JSON-LD

C'est l'innovation majeure de la version actuelle. La base de données ne stocke pas des objets "muets", mais des concepts liés à l'ontologie **Arcadia**.

### Cycle de Vie Sémantique

Lorsqu'un document est inséré via `insert_with_schema` :

1.  **Validation Structurelle** : Vérification contre le JSON Schema (champs requis, formats).
2.  **Enrichissement** : Injection automatique du `@context` par défaut si absent.
    ```json
    "@context": { "oa": "https://genaptitude.io/ontology/arcadia/oa#", ... }
    ```
3.  **Validation Sémantique** : Le `JsonLdProcessor` analyse le champ `@type`.
    - Il étend le terme (ex: `oa:Actor` -\> `https://...#OperationalActor`).
    - Il vérifie l'existence de ce concept dans le `VocabularyRegistry` (Code compilé).
    - Si le concept est inconnu, un warning est émis (ou une erreur en mode strict).

Cela garantit que toutes les données stockées sont conformes au méta-modèle métier.

---

## ⚡ Transactions Intelligentes

Le `TransactionManager` supporte deux modes de fonctionnement :

### 1\. Mode "Smart" (Haut Niveau)

Utilisé par le CLI et le Frontend. Il permet de décrire des **intentions** plutôt que des opérations brutes.

- **Résolution de Références** : Permet de cibler un document par une clé métier (ex: `handle`) plutôt que par son UUID. Le moteur effectue la recherche (`QueryEngine`) avant d'appliquer la modification.
- **Auto-Completion** : Génère les UUIDs manquants et injecte les métadonnées techniques.
- **Opérations supportées** : `Insert`, `Update` (avec Merge intelligent), `Delete`, `InsertFrom` (fichier).

### 2\. Mode ACID (Bas Niveau)

Assure la sécurité des données :

- **Isolation** : Verrouillage (`RwLock`) au niveau Collection.
- **Durabilité** : Écriture dans le WAL avant modification des fichiers de données.
- **Atomicité** : En cas d'erreur au milieu d'une transaction, un **Rollback** automatique restaure l'état précédent.

---

## 🔍 Moteur de Requête SQL

Le module `query` permet d'interroger la base avec une syntaxe SQL standard.

### Fonctionnalités

- **Projection** : `SELECT name, age` (renvoie uniquement les champs demandés).
- **Filtrage** : `WHERE kind = 'human' AND tags LIKE 'admin'`. Supporte les opérateurs logiques imbriqués.
- **Tri** : `ORDER BY createdAt DESC`.
- **Pagination** : Gestion interne via `limit` et `offset`.

### Exemple d'utilisation (Rust)

```rust
let q = parse_sql("SELECT handle, kind FROM actors WHERE kind = 'robot'")?;
let result = query_engine.execute_query(q).await?;

for doc in result.documents {
    println!("{}", doc); // {"handle": "robot-01", "kind": "robot"}
}
```

---

## 🚀 Indexation Automatique

Le moteur maintient automatiquement les index définis dans `_meta.json` lors des opérations CRUD (`insert`, `update`, `delete`).

- **Transparence** : L'utilisateur n'a pas à gérer les index manuellement.
- **Types supportés** :
  - `Hash` : Pour les recherches exactes (IDs, Handles).
  - `BTree` : Pour les tris et les plages de valeurs.
  - `Text` : Pour la recherche de mots-clés (tokenisation simple).

---

## 💡 Guide du Développeur

### Insertion d'un Document

```rust
use genaptitude::json_db::collections::manager::CollectionsManager;

let mgr = CollectionsManager::new(&storage, "un2", "_system");

let doc = json!({
    "@type": "oa:OperationalActor", // Sera validé sémantiquement
    "handle": "user-01",
    "displayName": "Utilisateur Test"
});

// 1. Calcul ID & Dates -> 2. Validation Schema -> 3. Validation Sémantique -> 4. Indexation -> 5. Disque
mgr.insert_with_schema("actors", doc)?;
```

### Exécution d'une Transaction Complexe

```rust
use genaptitude::json_db::transactions::{TransactionManager, TransactionRequest};

let tm = TransactionManager::new(&config, "un2", "_system");

let ops = vec![
    TransactionRequest::Update {
        collection: "actors".to_string(),
        id: None,
        handle: Some("admin".to_string()), // Résolution automatique
        document: json!({ "x_active": true }) // Merge partiel
    }
];

// Exécution asynchrone sécurisée
tm.execute_smart(ops).await?;
```

---

## ⚠️ Limitations Connues

1.  **Jointures** : Le moteur SQL ne supporte pas encore les `JOIN`.
2.  **Concurrence** : Le verrouillage est au niveau Collection (pas Document).
3.  **SQL Parser** : Le support de `LIMIT/OFFSET` en SQL pur est temporairement désactivé (utiliser l'API Rust `Query` struct).
