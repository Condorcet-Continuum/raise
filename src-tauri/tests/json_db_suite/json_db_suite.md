# 🧪 Suite de Tests `json_db` & `model_engine`

## Vue d'Ensemble

La suite de tests de GenAptitude est un ensemble complet de tests d'intégration validant le comportement du module `json_db` (stockage, requêtes, ACID) et du `model_engine` (sémantique, chargement).

### Caractéristiques de la Suite

- **Tests d'intégration** : Scénarios end-to-end (CRUD, Requêtes complexes).
- **Isolation complète** : Chaque test utilise un `TestEnv` avec un répertoire temporaire unique.
- **Couverture exhaustive** : Cycle de vie, ACID, x_compute, SQL, et Sémantique JSON-LD.
- **Datasets réels** : Utilisation des schémas Arcadia et données de test.

---

## 📁 Structure de la Suite

```text
tests/
├── json_db_suite.rs              # Point d'entrée de la suite d'intégration DB
└── json_db_suite/
    ├── mod.rs                     # Utilitaires communs (TestEnv, init)
    ├── json_db_lifecycle.rs       # Cycle de vie (Create/Drop Soft & Hard)
    ├── json_db_errors.rs          # Gestion d'erreurs (Doubles créations...)
    ├── json_db_idempotent.rs      # Idempotence des opérations
    ├── json_db_integration.rs     # CRUD basique et logique métier
    ├── json_db_query_integration.rs # Moteur de requêtes (Filtres JSON)
    ├── json_db_sql.rs             # NOUVEAU : Moteur SQL (Select, Where, Order)
    ├── workunits_x_compute.rs     # Calculs complexes (Finance, Dates, UUID)
    ├── schema_minimal.rs          # Validation minimale
    └── dataset_integration.rs     # NOUVEAU : Import de masse et datasets
```

En plus de la suite externe, des tests unitaires/intégration existent dans le code source :

```text
src-tauri/src/
└── model_engine/
    └── tests.rs                   # NOUVEAU : Tests sémantiques (OA/SA dispatch)
```

### Statistiques

| Fichier                        | Focus           | Nouveautés v1.1                      |
| :----------------------------- | :-------------- | :----------------------------------- |
| `json_db_lifecycle.rs`         | Cycle de vie DB | Tests de persistance des schémas     |
| `json_db_integration.rs`       | CRUD            | Tests d'insertion avec validation    |
| `json_db_query_integration.rs` | Moteur Query    | Filtres complexes et tris            |
| `json_db_sql.rs`               | **Moteur SQL**  | Parsing SQL, clauses WHERE/ORDER     |
| `workunits_x_compute.rs`       | x_compute       | Calculs financiers et agrégats       |
| `dataset_integration.rs`       | **Datasets**    | Chargement de fichiers externes      |
| `model_engine/tests.rs`        | **Sémantique**  | Validation JSON-LD et typage Arcadia |

---

## 🔧 Module Commun (`mod.rs`)

### `init_test_env()`

Fonction d'initialisation robuste utilisée par tous les tests.

1.  **Isolation** : Crée un `TempDir` unique.
2.  **Seeding** : Copie les schémas réels (`schemas/v1`) dans l'environnement de test.
3.  **Configuration** : Instancie un `StorageEngine` thread-safe.

<!-- end list -->

```rust
let env = init_test_env();
// env.cfg pointe vers le dossier temporaire peuplé
// env.storage est prêt à l'emploi
```

---

## 📝 Tests Détaillés

### 1\. Tests Moteur SQL (`json_db_sql.rs`)

Valide le parser et l'exécuteur SQL expérimental.

- **`test_sql_select_by_kind`** : `SELECT * FROM actors WHERE kind = 'bot'`
- **`test_sql_numeric_comparison`** : Filtres sur propriétés étendues (`x_age >= 30`)
- **`test_sql_like`** : Recherche textuelle (`displayName LIKE 'User'`)
- **`test_sql_json_array`** : Filtre dans les tableaux (`tags LIKE 'paris'`)

### 2\. Tests x_compute (`workunits_x_compute.rs`)

Valide le moteur de règles de calcul avant insertion.

- **`workunit_compute`** : Génération automatique d'UUID, `createdAt`, injection `$schema`.
- **`finance_compute`** : Calcul de totaux (`total_eur = prix * volume`) et agrégats complexes définis dans le schéma JSON.

### 3\. Tests Model Engine (`src/model_engine/tests.rs`)

Ce test est crucial pour la couche sémantique.

- **`test_semantic_loading_oa_and_sa`** :
  - Insère des documents JSON-LD bruts (avec `@context` et `@type`).
  - Charge le projet via `ModelLoader`.
  - Vérifie que :
    - Un `@type: oa:OperationalActor` devient un objet dans `model.oa.actors`.
    - Un `@type: sa:SystemFunction` devient un objet dans `model.sa.functions`.
    - Les URIs sont correctement étendues (`https://...`).

### 4\. Tests Dataset (`dataset_integration.rs`)

Vérifie l'importation de données de référence.

- **`debug_import_exchange_item`** : Charge un fichier JSON externe, valide son schéma, et l'insère en base. Simule le comportement de la CLI `import`.

---

## 🚀 Exécution des Tests

### Suite JSON-DB (Stockage & Requêtes)

```bash
# Lancer tous les tests de la suite d'intégration
cargo test --test json_db_suite

# Lancer uniquement les tests SQL
cargo test --test json_db_suite -- json_db_sql
```

### Tests Model Engine (Sémantique)

Ces tests sont situés dans la librairie principale (`src/lib.rs`).

```bash
# Lancer les tests du Model Engine
cargo test --package genaptitude --lib model_engine::tests -- --nocapture
```

---

## ✅ Bonnes Pratiques Ajoutées

- **Utilisation de `from_engine`** : Les tests du `ModelLoader` utilisent un constructeur découplé pour éviter de mocker l'état Tauri complexe.
- **Préparation des Données** : Les tests insèrent désormais des documents valides par rapport aux schémas (ex: structure `finance` complète) pour passer la validation stricte.
- **Nettoyage** : Le `TempDir` assure qu'aucun fichier de test ne persiste après l'exécution (sauf en cas de panic si configuré pour le debug).
