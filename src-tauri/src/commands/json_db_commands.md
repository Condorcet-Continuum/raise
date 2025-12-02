# Commandes Tauri : json_db

> **Version API :** 1.1 (Stable)  
> **Statut :** Implémenté & Testé

Cette documentation détaille l'API Tauri exposée pour la base de données JSON. Ces commandes sont asynchrones et thread-safe, utilisant le `StorageEngine` partagé.

---

## 🔌 Vue d'Ensemble

Toutes les commandes doivent être appelées via `invoke` côté Frontend.

Le backend gère automatiquement :

1. **La sélection du Space/DB** (ex: `un2/_system`)
2. **Le chargement des schémas** (si présents dans `_meta.json`)
3. **Le calcul des champs** (`x_compute`: ID, dates)
4. **La validation** avant écriture

---

## 1. Gestion des Collections

### `jsondb_create_collection`

Crée une nouvelle collection (dossier sur le disque) et y associe optionnellement un schéma JSON.

**Signature Rust :**

```rust
async fn jsondb_create_collection(
    storage: State<StorageEngine>,
    space: String,
    db: String,
    collection: String,
    schema_uri: Option<String>
) -> Result<(), String>
```

**Usage TypeScript :**

```typescript
await invoke('jsondb_create_collection', {
  space: 'un2',
  db: '_system',
  collection: 'articles',
  schemaUri: 'db://un2/_system/schemas/v1/article.json', // ou null
});
```

---

### `jsondb_list_collections`

Retourne la liste des noms de collections existantes dans la base donnée.

**Signature Rust :**

```rust
async fn jsondb_list_collections(
    storage: State<StorageEngine>,
    space: String,
    db: String
) -> Result<Vec<String>, String>
```

**Usage TypeScript :**

```typescript
const collections = await invoke<string[]>('jsondb_list_collections', {
  space: 'un2',
  db: '_system',
});
// ["articles", "users", "logs"]
```

---

## 2. Documents (CRUD)

### `jsondb_insert_document`

Insère un document.

- Si le document n'a pas d'ID, un UUID v4 est généré
- Si un schéma est lié, les règles `x_compute` sont appliquées (timestamps, slugs, etc.)

**Signature Rust :**

```rust
async fn jsondb_insert_document(
    storage: State<StorageEngine>,
    space: String,
    db: String,
    collection: String,
    document: serde_json::Value
) -> Result<Value, String>
```

**Usage TypeScript :**

```typescript
const doc = { title: 'Mon Article' }; // Pas besoin d'ID
const saved = await invoke('jsondb_insert_document', {
  space: 'un2',
  db: '_system',
  collection: 'articles',
  document: doc,
});
console.log(saved.id); // L'ID généré est retourné
```

---

### `jsondb_update_document`

Met à jour un document existant. L'ID doit correspondre.

**Signature Rust :**

```rust
async fn jsondb_update_document(
    storage: State<StorageEngine>,
    space: String,
    db: String,
    collection: String,
    id: String,
    document: Value
) -> Result<Value, String>
```

---

### `jsondb_get_document`

Récupère un document par son ID exact.

**Signature Rust :**

```rust
async fn jsondb_get_document(
    storage: State<StorageEngine>,
    space: String,
    db: String,
    collection: String,
    id: String
) -> Result<Option<Value>, String>
```

---

### `jsondb_delete_document`

Supprime physiquement un document.

**Signature Rust :**

```rust
async fn jsondb_delete_document(
    storage: State<StorageEngine>,
    space: String,
    db: String,
    collection: String,
    id: String
) -> Result<bool, String>
```

---

## 3. Recherche & Requêtes (Engine)

C'est ici que réside la puissance du moteur. Vous avez trois méthodes pour interroger les données.

### `jsondb_list_all`

Récupère tous les documents d'une collection sans filtre.

⚠️ **À utiliser avec parcimonie sur les grosses collections**

**Signature Rust :**

```rust
async fn jsondb_list_all(
    storage: State<StorageEngine>,
    space: String,
    db: String,
    collection: String
) -> Result<Vec<Value>, String>
```

---

### `jsondb_execute_query` (JSON Query)

Exécute une recherche structurée (Filtres, Tri, Pagination). Idéal pour construire des UI complexes.

**Signature Rust :**

```rust
async fn jsondb_execute_query(
    storage: State<StorageEngine>,
    space: String,
    db: String,
    query: Query
) -> Result<QueryResult, String>
```

**Structure de l'objet Query (TypeScript) :**

```typescript
interface Query {
  collection: string;
  filter?: {
    operator: 'And' | 'Or' | 'Not';
    conditions: Array<{
      field: string;
      operator: 'Eq' | 'Contains' | 'Gt' | 'Gte' | 'Lt' | 'Lte'; // PascalCase côté Rust
      value: any;
    }>;
  };
  sort?: Array<{ field: string; order: 'Asc' | 'Desc' }>;
  limit?: number;
  offset?: number;
}
```

---

### `jsondb_execute_sql` (SQL)

Exécute une requête via une syntaxe SQL standard.

**Note :** Les clauses `LIMIT` et `OFFSET` sont temporairement ignorées par le parser SQL actuel pour des raisons de compatibilité, mais le `WHERE` (filtrage) et `ORDER BY` (tri) fonctionnent parfaitement.

**Signature Rust :**

```rust
async fn jsondb_execute_sql(
    storage: State<StorageEngine>,
    space: String,
    db: String,
    sql: String
) -> Result<QueryResult, String>
```

**Usage TypeScript :**

```typescript
const result = await invoke('jsondb_execute_sql', {
  space: 'un2',
  db: '_system',
  sql: "SELECT * FROM articles WHERE status = 'published' ORDER BY updatedAt DESC",
});
```

---

## 4. Résumé des Types de Retour

| Commande            | Type Retour (TS) | Description                                      |
| ------------------- | ---------------- | ------------------------------------------------ |
| `create_collection` | `void`           | Succès ou erreur string                          |
| `list_collections`  | `string[]`       | Liste des noms                                   |
| `insert_document`   | `Document`       | Le document complet tel que sauvegardé (avec ID) |
| `update_document`   | `Document`       | Le document mis à jour                           |
| `execute_query`     | `QueryResult`    | `{ documents: any[], total: number }`            |
| `execute_sql`       | `QueryResult`    | `{ documents: any[], total: number }`            |
| `list_all`          | `Document[]`     | Liste brute des documents                        |

---

**Fin de la documentation**
