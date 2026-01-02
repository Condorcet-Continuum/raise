# Module Query (JSON-DB)

Ce module implémente le moteur de requêtes de RAISE. Il permet d'interroger les collections JSON en utilisant soit un constructeur fluide (Fluent Builder), soit une syntaxe SQL standard, avec support de l'optimisation et de la pagination.

## 🏗️ Architecture

Le moteur de requête est structuré en pipeline classique de base de données :

1.  **Parsing** (`parser.rs`, `sql.rs`) : Transforme la requête (JSON ou SQL) en un objet `Query` structuré interne.
2.  **Optimisation** (`optimizer.rs`) : Réorganise et simplifie la requête pour améliorer les performances (ex: exécuter les filtres les plus sélectifs en premier).
3.  **Exécution** (`executor.rs`) : Lit les données via le `CollectionsManager`, applique les filtres, le tri, la pagination et la projection.

## 🚀 Fonctionnalités Clés

### 1\. Parsing SQL (`sql.rs`)

Le module intègre un parseur SQL complet (basé sur `sqlparser`) permettant d'écrire des requêtes naturelles.

- **SELECT** : Supporte les projections (`SELECT name, age`), les alias (`SELECT u.name`), et le wildcard (`SELECT *`).
- **WHERE** : Supporte les opérateurs logiques (`AND`, `OR`), de comparaison (`=`, `!=`, `>`, `<`, `>=`, `<=`) et textuels (`LIKE`).
- **ORDER BY** : Tri multi-critères (`ORDER BY age DESC, name ASC`).
- **LIMIT / OFFSET** : Pagination standard (bien que temporairement désactivée dans le traducteur SQL, elle est supportée par le moteur interne).

### 2\. Optimiseur de Requêtes (`optimizer.rs`)

Avant exécution, chaque requête passe par l'optimiseur `QueryOptimizer` qui applique plusieurs stratégies:

- **Réorganisation des Conditions (Sélectivité)** : Les filtres sont triés par coût estimé. Une égalité stricte (`Eq`, coût 1) sera vérifiée avant une recherche textuelle (`Contains`, coût 50) ou une négation (`Ne`, coût 100). Cela permet d'éliminer les documents non correspondants le plus tôt possible ("Fail Fast").
- **Simplification** : Déduplication des conditions redondantes.
- **Optimisation Pagination** : Plafonnement automatique des `LIMIT` excessifs (\> 1000) pour éviter les scans mémoire trop lourds.

### 3\. Exécution (`executor.rs`)

L'`Executor` orchestre le traitement des données en mémoire (pour l'instant, chargement complet de la collection).

- **Filtrage** : Évaluation récursive des prédicats `QueryFilter` sur les documents JSON. Supporte les chemins imbriqués (ex: `address.city`) via pointeurs JSON.
- **Projection** : Sélectionne uniquement les champs demandés (`Include`) ou exclut des champs sensibles (`Exclude`), reconstruisant un nouvel objet JSON propre.
- **Comparaison** : Gestion robuste des types JSON (comparaison nombre vs nombre, chaîne vs chaîne) avec gestion du `null` (considéré inférieur à toute valeur).

## 🛠️ Utilisation

### Via SQL (Recommandé)

```rust
use crate::json_db::query::sql::parse_sql;

let sql = "SELECT name, email FROM users WHERE age > 18 AND role = 'admin' ORDER BY created_at DESC";
let query = parse_sql(sql)?;
let result = engine.execute_query(query).await?;
```

### Via QueryBuilder (Programmatique)

```rust
use crate::json_db::query::parser::QueryBuilder;

let query = QueryBuilder::new("users")
    .where_eq("status", json!("active"))
    .select(vec!["username".to_string()])
    .build();

let result = engine.execute_query(query).await?;
```

## 📂 Structure des Fichiers

```text
src-tauri/src/json_db/query/
├── mod.rs          // Définitions des structures (Query, Filter, Condition)
├── sql.rs          // Traducteur SQL -> Query interne
├── parser.rs       // Helpers pour le parsing JSON et Builder
├── optimizer.rs    // Logique d'optimisation (Sélectivité, Simplification)
└── executor.rs     // Moteur d'exécution (Scan, Filter, Sort, Project)
```

## ⚠️ Limitations Actuelles

- **Full Scan** : L'exécuteur charge **tous** les documents de la collection en mémoire (`manager.list_all`) avant de filtrer. Il n'utilise pas encore les index (`json_db/indexes`) pour accélérer la recherche, ce qui est la prochaine étape d'optimisation majeure.
- **Joins** : Les requêtes SQL ne supportent qu'une seule table (`FROM users`). Les jointures (`JOIN`) ne sont pas implémentées.
- **Agrégations** : Pas de support pour `GROUP BY`, `COUNT`, `SUM`, etc.
