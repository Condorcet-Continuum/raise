# JSON-DB Client SDK 🗄️

Ce module fournit une couche de service ("Bridge") pour interagir avec le moteur de base de données **GenAptitude JSON-DB** (implémenté en Rust).
Il expose des méthodes typées pour gérer les collections, exécuter des requêtes NoSQL/SQL et gérer des transactions atomiques via l'IPC Tauri.

---

## 📂 Inventaire des Services

| Fichier                      | Rôle                                                                                                                                          |
| ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| **`collection-service.ts`**  | Gestionnaire principal : CRUD de documents, création/suppression de bases et de collections, gestion des index.                               |
| **`query-service.ts`**       | Constructeur de requêtes (`QueryBuilder`) et exécutant. Supporte la syntaxe objet (NoSQL) et le SQL brut.                                     |
| **`transaction-service.ts`** | Permet d'empiler plusieurs opérations (Insert, Update, Delete) et de les exécuter séquentiellement ("Commit") ou de les annuler ("Rollback"). |
| **`jsonld-service.ts`**      | Utilitaire client pour manipuler les contextes sémantiques (JSON-LD), utile pour l'interopérabilité des modèles.                              |
| **`schema-service.ts`**      | Gestion des URI de schémas JSON pour la validation des données.                                                                               |

---

## ⚙️ Configuration Dynamique

Contrairement à une API REST classique avec une URL fixe, ce SDK est configuré dynamiquement via le **Global State** (`settings-store`).

À chaque appel (ex: `listAll`), le service :

1.  Interroge `useSettingsStore` pour connaître l'**Espace** (`space`) et la **Base** (`db`) actifs.
2.  Envoie ces paramètres au backend Rust via `invoke`.

Cela permet de changer de contexte de base de données à la volée dans l'interface sans recharger l'application.

---

## 💻 Exemples d'utilisation

### 1. Opérations de Base (CRUD)

```typescript
import { collectionService } from '@/services/json-db';

// Créer une collection
await collectionService.createCollection('users');

// Insérer un document
await collectionService.insertDocument('users', {
  id: 'u1',
  name: 'Alice',
  role: 'architect',
});

// Récupérer tout
const users = await collectionService.listAll('users');
```

### 2\. Requêtes Avancées (QueryBuilder)

Utilisation du pattern "Builder" pour construire des filtres lisibles.

```typescript
import { createQuery, queryService } from '@/services/json-db';

// Construction de la requête
const query = createQuery('projects')
  .where('status', 'Eq', 'active')
  .where('budget', 'Gt', 5000)
  .orderBy('createdAt', 'Desc')
  .limit(10)
  .build();

// Exécution
const results = await queryService.execute(query);
```

### 3\. Transactions

Permet de grouper des modifications.

```typescript
import { createTransaction } from '@/services/json-db';

const tx = createTransaction();

tx.add('logs', { msg: 'Début traitement', level: 'info' })
  .update('users', 'u1', { lastLogin: Date.now() })
  .delete('cache', 'temp-key-123');

// Envoi en une fois au backend
await tx.commit();
```

---

## 🔗 Correspondance Backend (Rust)

Ces services appellent les commandes Tauri définies dans le backend Rust (`src-tauri/src/commands/jsondb.rs`).
Les signatures doivent rester synchronisées.

| Commande TS        | Commande Rust (`invoke`)   |
| ------------------ | -------------------------- |
| `createCollection` | `jsondb_create_collection` |
| `insertDocument`   | `jsondb_insert_document`   |
| `executeQuery`     | `jsondb_execute_query`     |
| `executeSql`       | `jsondb_execute_sql`       |

---

## 🛠️ Maintenance

- **Types :** Les interfaces (`Query`, `Document`, `OperationRequest`) sont définies dans `@/types/json-db.types.ts`.
- **Erreurs :** Les erreurs Rust sont propagées sous forme de chaînes de caractères (String) via la Promise rejetée. Pensez à les `try/catch`.
