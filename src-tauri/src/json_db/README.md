# Architecture JSON-DB (RAISE)

**JSON-DB** est le moteur de base de données embarqué, orienté document et sémantique, développé spécifiquement pour RAISE. Il combine la simplicité du stockage de fichiers JSON plats avec la robustesse d'une base de données transactionnelle (ACID) et la puissance du Web Sémantique (JSON-LD).

## 🌍 Vue d'Ensemble

Le système est conçu en couches modulaires, allant du stockage physique bas niveau jusqu'à l'orchestration transactionnelle de haut niveau.

### Principes Clés

- **Stockage Texte** : Chaque document est un fichier `.json` lisible et éditable par un humain.
- **Architecture Sémantique** : Intégration native de JSON-LD pour lier les données à l'ontologie Arcadia (`oa:`, `sa:`, `la:`, etc.).
- **Intégrité ACID** : Support des transactions multi-collections avec journalisation (WAL) et verrouillage.
- **Évolutionnaire** : Système de **Migrations** intégré pour faire évoluer la structure des données sans casser l'existant.
- **Requêtes SQL** : Moteur de recherche supportant une syntaxe SQL standard pour filtrer et trier les données JSON.

---

## 📂 Arborescence du Code Source

Voici la structure exhaustive des modules et fichiers composants le moteur :

```text
src-tauri/src/json_db/
├── mod.rs                  // Point d'entrée du module global
├── README.md               // Documentation générale (ce fichier)
├── collections/            // Gestion des collections et cycle de vie
│   ├── mod.rs
│   ├── manager.rs          // Orchestrateur (Règles + Validation + Indexation)
│   └── collection.rs       // Opérations I/O bas niveau
├── indexes/                // Moteur d'indexation
│   ├── mod.rs
│   ├── manager.rs          // Cycle de vie des index (Create/Drop)
│   ├── driver.rs           // Abstraction I/O
│   ├── hash.rs             // Index Hash (Egalité stricte)
│   ├── btree.rs            // Index BTree (Plages/Tri)
│   └── text.rs             // Index Inversé (Recherche plein texte)
├── jsonld/                 // Moteur sémantique
│   ├── mod.rs
│   ├── processor.rs        // Algorithmes Expansion/Compaction/RDF
│   ├── context.rs          // Gestion des contextes (@context)
│   └── vocabulary.rs       // Registre statique Arcadia
├── migrations/             // [NOUVEAU] Gestion des versions de schéma
│   ├── mod.rs
│   ├── migrator.rs         // Moteur d'exécution des migrations (Up/Down)
│   └── version.rs          // Gestion Semantic Versioning
├── query/                  // Moteur de recherche
│   ├── mod.rs
│   ├── sql.rs              // Parsing SQL
│   ├── parser.rs           // Parsing JSON Query
│   ├── optimizer.rs        // Optimisation (Sélectivité)
│   └── executor.rs         // Exécution (Scan, Filter, Sort)
├── schema/                 // Validation structurelle
│   ├── mod.rs
│   ├── registry.rs         // Chargement et cache des schémas
│   └── validator.rs        // Validation JSON Schema (Draft 2020-12 subset)
├── storage/                // Persistance physique
│   ├── mod.rs
│   ├── file_storage.rs     // I/O atomique
│   └── cache.rs            // Cache LRU thread-safe
├── transactions/           // Moteur ACID
│   ├── mod.rs
│   ├── manager.rs          // Gestionnaire (Execute, Commit)
│   ├── wal.rs              // Write-Ahead Log (Journalisation)
│   └── lock_manager.rs     // Gestion des verrous
└── test_utils.rs           // [NOUVEAU] Outillage de tests d'intégration

```

---

## 🧩 Modules du Système

### 1. Storage (`src/json_db/storage`)

**La Couche Physique.**
Gère l'interaction avec le système de fichiers.

- **Sécurité** : Utilise des écritures atomiques (fichier `.tmp` + rename) pour éviter la corruption.
- **Performance** : Intègre un cache LRU thread-safe pour accélérer les lectures fréquentes.

### 2. Collections (`src/json_db/collections`)

**L'Orchestrateur.**
La façade principale pour manipuler les données.

- **Rôle** : Coordonne le cycle de vie d'un document. C'est ici que réside le moteur de règles **GenRules**.
- **Pipeline** : Injection ID -> Règles Métier -> Validation Schema -> Enrichissement Sémantique -> Persistance.

### 3. Migrations (`src/json_db/migrations`) 🆕

**L'Évolution du Schéma.**
Permet de modifier la structure de la base de données de manière contrôlée.

- **Versionning** : Utilise _Semantic Versioning_ pour ordonner les mises à jour.
- **Traçabilité** : Stocke l'historique des migrations appliquées dans la collection système `_migrations`.
- **Opérations** : Supporte `CreateCollection`, `AddField`, `RenameField`, etc.

### 4. Transactions (`src/json_db/transactions`)

**La Sécurité des Données.**
Gère les opérations atomiques complexes.

- **ACID** : Utilise un Write-Ahead Log (WAL) pour garantir la durabilité et un LockManager pour l'isolation.
- **Smart API** : Offre des méthodes de haut niveau pour gérer les insertions massives.

### 5. Schema (`src/json_db/schema`)

**La Validation Structurelle.**

- **Rôle** : Validation JSON Schema (Draft 2020-12).
- **Features** : Résolution des références `$ref` via un registre central (`db://...`).

### 6. JSON-LD (`src/json_db/jsonld`)

**Le Moteur Sémantique.**

- **Rôle** : Expansion/Compaction des clés et validation ontologique.
- **Ontologie** : Embarque les définitions Arcadia (OA, SA, LA, PA, EPBS, DATA).

### 7. Query & Indexes (`src/json_db/query`, `src/json_db/indexes`)

**L'Accès aux Données.**

- **Query** : Supporte SQL (`SELECT * FROM users WHERE age > 18`) et un QueryBuilder.
- **Indexes** : Hash, BTree et Text, mis à jour atomiquement lors des transactions.

---

## 🧪 Stratégie de Test (`src/json_db/test_utils.rs`)

Pour garantir la fiabilité sans corrompre les données de développement, le module fournit un environnement de test isolé via `TestEnv`.

### Fonctionnement de `TestEnv`

1. **Isolation** : Crée un répertoire temporaire (`tempfile`) qui sera détruit à la fin du test.
2. **Clonage des Schémas** : Copie récursivement les schémas réels (`schemas/v1`) vers l'environnement temporaire pour valider les tests avec la vraie logique métier.
3. **Mocking** : Génère des datasets factices (ex: `mock-article`) pour simuler une base pré-remplie.

**Exemple d'utilisation dans un test :**

```rust
#[test]
fn test_my_feature() {
    // Initialise l'environnement (Logs + Temp Dir + Schémas)
    let env = crate::json_db::test_utils::init_test_env();

    // On utilise env.storage et env.space pour les opérations
    let mgr = CollectionsManager::new(&env.storage, &env.space, &env.db);
    // ... assertions ...
}

```

---

## 🔄 Flux de Données (Pipeline d'Écriture)

Lorsqu'une transaction `Insert` ou `Update` est soumise, le document traverse le pipeline suivant :

1. **Transaction Manager** : Acquiert les verrous et écrit l'intention dans le WAL.
2. **Collections Manager** : Prépare le document (injection ID/Dates).
3. **GenRules Engine** : Exécute les règles métier (`x_rules`) pour calculer les champs dérivés.
4. **Schema Validator** : Vérifie la structure stricte du document.
5. **JSON-LD Processor** : Vérifie la cohérence sémantique.
6. **Storage Engine** : Écrit le fichier JSON atomiquement sur le disque.
7. **Index Manager** : Met à jour les index (Hash, BTree, Text).
8. **Commit** : Nettoyage du WAL et libération des verrous.

---

## 🛠️ Exemple d'Utilisation Globale

```rust
use crate::json_db::storage::JsonDbConfig;
use crate::json_db::transactions::{TransactionManager, TransactionRequest};
use crate::json_db::query::sql::parse_sql;
use crate::json_db::query::QueryEngine;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use serde_json::json;

async fn demo() -> anyhow::Result<()> {
    let config = JsonDbConfig::new("/tmp/raise_data");
    let space = "demo_space";
    let db = "demo_db";

    // 1. Transaction : Insertion sécurisée
    let tx_mgr = TransactionManager::new(&config, space, db);
    tx_mgr.execute_smart(vec![
        TransactionRequest::Insert {
            collection: "users".to_string(),
            id: None,
            document: json!({
                "name": "Alice",
                "role": "admin",
                "age": 30
            }),
        }
    ]).await?;

    // 2. Requête : Recherche SQL
    let sql = "SELECT name, age FROM users WHERE role = 'admin' ORDER BY age DESC";
    let query = parse_sql(sql)?;

    // 3. Exécution
    let storage = StorageEngine::new(config.clone());
    let col_mgr = CollectionsManager::new(&storage, space, db);
    let engine = QueryEngine::new(&col_mgr);

    let result = engine.execute_query(query).await?;

    println!("Résultats : {:?}", result.documents);
    Ok(())
}

```
