# 🚀 GenAptitude JSON-DB CLI

**`jsondb_cli`** est l'outil d'administration en ligne de commande pour la base de données JSON locale de GenAptitude. Il permet de gérer le cycle de vie des bases de données, des collections, et de manipuler les documents avec validation de schéma stricte.

---

## 📋 Prérequis et Configuration

Avant d'utiliser le CLI, assurez-vous que votre environnement est configuré. Le CLI s'appuie sur des variables d'environnement pour localiser le stockage et les schémas sources.

### Fichier `.env` (Racine du projet)

Ces variables sont **obligatoires** :

```bash
# Racine du stockage physique (Dossier où les données seront écrites)
PATH_GENAPTITUDE_DOMAIN="/home/votre_user/genaptitude_domain"

# Chemin relatif vers le schéma maître (utilisé lors du bootstrap create-db)
GENAPTITUDE_DB_SCHEMA="schemas/v1/db/index.schema.json"
```

---

## 🛠️ Usage Général

```bash
cargo run -p jsondb_cli -- [OPTIONS_GLOBALES] <COMMANDE> [ARGS]
```

### Options Globales

| Option    | Alias | Défaut          | Description                                           |
| :-------- | :---- | :-------------- | :---------------------------------------------------- |
| `--space` | `-s`  | `default_space` | L'espace de noms logique (Tenant). Ex: `un2`.         |
| `--db`    | `-d`  | `default_db`    | Le nom de la base de données. Ex: `_system`.          |
| `--root`  |       | _via ENV_       | Surcharge le chemin racine `PATH_GENAPTITUDE_DOMAIN`. |

---

## 📦 Gestion du Cycle de Vie (Base de Données)

### `create-db`

Initialise une nouvelle base de données.

- Crée l'arborescence physique.
- **Bootstrap** : Copie les schémas sources (`schemas/v1`) vers le dossier de la base.
- **Index** : Génère `_system.json` à partir du schéma maître (`index.schema.json`) en peuplant les définitions par défaut.
- **Collections** : Crée physiquement les dossiers pour toutes les collections définies dans l'index.

<!-- end list -->

```bash
# Exemple : Création complète de l'environnement
cargo run -p jsondb_cli -- --space un2 --db _system create-db
```

### `drop-db`

Supprime ou archive une base de données.

- **Mode "Soft" (Défaut)** : Renomme le dossier en `.deleted-<timestamp>`. Permet la restauration.
- **Mode "Hard" (`--force`)** : Suppression définitive du disque.

<!-- end list -->

```bash
# Archivage (Sécurité)
cargo run -p jsondb_cli -- --space un2 --db _system drop-db

# Suppression totale (Pour les tests/dev)
cargo run -p jsondb_cli -- --space un2 --db _system drop-db --force
```

---

## 📂 Gestion des Collections

### `create-collection`

Crée une collection et son fichier de métadonnées `_meta.json`.

**Mode Intelligent :**
Si vous ne fournissez pas de schéma, le CLI le cherche automatiquement dans `_system.json`.

- Si trouvé : Il résout l'URI absolue (`db://...`) et crée la collection.
- Si non trouvé : Il rejette la création par sécurité.

**Mode Explicite :**
Vous pouvez forcer un schéma spécifique avec `--schema`.

```bash
# 1. Mode Automatique (Recommandé si défini dans l'index)
cargo run -p jsondb_cli -- --space un2 --db _system create-collection actors

# 2. Mode Manuel
cargo run -p jsondb_cli -- --space un2 --db _system create-collection custom_logs --schema "db://un2/_system/schemas/v1/logs/log.schema.json"
```

### `list-collections`

Liste les collections physiquement présentes sur le disque.

```bash
cargo run -p jsondb_cli -- --space un2 --db _system list-collections
```

---

## 📝 Manipulation de Données (CRUD)

### `insert`

Insère un document JSON dans une collection.

- **Injection Automatique** : Génère `id` (UUID v4) si manquant.
- **Injection Schéma** : Injecte le champ `$schema` automatiquement avant validation (permet au moteur `x_compute` de fonctionner correctement).
- **Validation** : Valide les données contre le schéma JSON associé.

**Via JSON en ligne :**

```bash
cargo run -p jsondb_cli -- --space un2 --db _system insert actors '{
  "handle": "dev-user",
  "displayName": "Développeur",
  "kind": "human"
}'
```

**Via Fichier (`@`) :**

```bash
cargo run -p jsondb_cli -- --space un2 --db _system insert actors @./mon_acteur.json
```

### `list-all`

Affiche tous les documents d'une collection (dump brut).

```bash
cargo run -p jsondb_cli -- --space un2 --db _system list-all actors
```

### `import`

Importe en masse un fichier ou tout un dossier de fichiers JSON.

```bash
# Import dossier
cargo run -p jsondb_cli -- --space un2 --db _system import actors ./data_source/actors/
```

---

## 🔍 Recherche (Query & SQL)

### `sql`

Exécute une requête SQL-like sur les fichiers JSON.
_Supporte `WHERE`, `ORDER BY`, `LIMIT` (partiel)._

```bash
cargo run -p jsondb_cli -- --space un2 --db _system sql --query "SELECT * FROM actors WHERE kind = 'human' AND tags LIKE 'core'"
```

### `query`

Interface bas niveau pour le moteur de requête (JSON Filter).

```bash
cargo run -p jsondb_cli -- --space un2 --db _system query actors --limit 5
```

---

## 🔄 Transactions

### `transaction`

Exécute une série d'opérations atomiques (ACID) définies dans un fichier JSON. Supporte le WAL (Write Ahead Log).

Exemple de fichier `tx.json` :

```json
{
  "operations": [
    {
      "type": "insert",
      "collection": "actors",
      "id": "new-uuid",
      "document": { ... }
    },
    {
      "type": "delete",
      "collection": "old_actors",
      "id": "old-uuid"
    }
  ]
}
```

Commande :

```bash
cargo run -p jsondb_cli -- --space un2 --db _system transaction ./tx.json
```

---

## ⚠️ Dépannage Courant

**Erreur : "Variable ENV manquante"**

> Vérifiez que vous avez bien un fichier `.env` à la racine du projet et que `cargo` est lancé depuis la racine.

**Erreur : "Schéma introuvable sur le disque"**

> Le fichier référencé dans `_system.json` ou via `--schema` n'existe pas physiquement dans `data/<space>/<db>/schemas/v1/`. Vérifiez votre bootstrap (`create-db`).

**Erreur : "Collection inconnue dans \_system.json"**

> Vous essayez de créer une collection sans schéma explicite, et elle n'est pas prévue dans le schéma maître. Utilisez `--schema` ou ajoutez la définition dans l'index.
