# 🚀 RAISE JSON-DB CLI

> **Version :** 1.2 (Décembre 2025)
> **Nouveautés :** Transactions Intelligentes, Moteur SQL avec Projections, Enrichissement Sémantique.

1.  **Transactions Intelligentes** : Résolution de `handle`, ID auto-générés, support de `update` avec merge.
2.  **Moteur SQL Avancé** : Support des projections (`SELECT a, b`) et des filtres complexes.
3.  **Sémantique** : Mention de l'enrichissement JSON-LD automatique lors de l'insertion.

**`jsondb_cli`** est l'outil d'administration en ligne de commande pour la base de données JSON locale de RAISE. Il permet de gérer le cycle de vie des bases de données, des collections, et de manipuler les documents avec une validation de schéma stricte et une cohérence sémantique.

---

## 📋 Prérequis et Configuration

Le CLI nécessite un environnement correctement configuré via un fichier `.env` à la racine du projet.

```bash
# Racine du stockage physique (Dossier où les données seront écrites)
PATH_RAISE_DOMAIN="/home/votre_user/raise_domain"

# Chemin relatif vers le schéma maître (utilisé lors du bootstrap create-db)
RAISE_DB_SCHEMA="schemas/v1/db/index.schema.json"
```

---

## 🛠️ Usage Général

```bash
cargo run -p jsondb_cli -- [OPTIONS_GLOBALES] <COMMANDE> [ARGS]
```

### Options Globales

| Option    | Alias | Défaut          | Description                                     |
| :-------- | :---- | :-------------- | :---------------------------------------------- |
| `--space` | `-s`  | `default_space` | L'espace de noms logique (Tenant). Ex: `un2`.   |
| `--db`    | `-d`  | `default_db`    | Le nom de la base de données. Ex: `_system`.    |
| `--root`  |       | _via ENV_       | Surcharge le chemin racine `PATH_RAISE_DOMAIN`. |

---

## 📦 Gestion du Cycle de Vie (Base de Données)

### `create-db`

Initialise une nouvelle base de données complète.

- **Physique** : Crée l'arborescence de dossiers.
- **Bootstrap** : Copie les schémas sources (`schemas/v1`) vers le stockage.
- **Index Système** : Génère `_system.json` à partir du schéma maître.
- **Collections** : Initialise toutes les collections définies dans l'index.

<!-- end list -->

```bash
cargo run -p jsondb_cli -- --space un2 --db _system create-db
```

### `drop-db`

Supprime ou archive une base de données.

- **Mode "Soft" (Défaut)** : Renomme le dossier en `.deleted-<timestamp>`.
- **Mode "Hard" (`--force`)** : Suppression irréversible.

<!-- end list -->

```bash
cargo run -p jsondb_cli -- --space un2 --db _system drop-db --force
```

---

## 📂 Gestion des Collections

### `create-collection`

Crée une collection, son dossier et son fichier de configuration `_meta.json`.

**Mode Intelligent (Recommandé) :**
Le CLI détecte automatiquement le schéma associé via `_system.json`.

```bash
cargo run -p jsondb_cli -- --space un2 --db _system create-collection actors
```

**Mode Manuel :**
Force un schéma spécifique via une URI absolue.

```bash
cargo run -p jsondb_cli -- --space un2 --db _system create-collection logs --schema "db://.../log.schema.json"
```

---

## 📝 Manipulation de Données (CRUD)

### `insert`

Insère un document JSON. Cette commande déclenche toute la pipeline "Intelligente" :

1.  **Injection ID** : Génère un UUID v4 si absent.
2.  **Enrichissement Sémantique** : Ajoute `@context` pour le JSON-LD.
3.  **Validation** : Vérifie la conformité au schéma et à l'ontologie Arcadia.
4.  **Indexation** : Met à jour les index (Hash, BTree) en temps réel.

<!-- end list -->

```bash
cargo run -p jsondb_cli -- --space un2 --db _system insert actors '{
  "handle": "dev-user",
  "displayName": "Développeur",
  "kind": "human"
}'
```

### `import`

Importe en masse un fichier ou un dossier complet.

```bash
cargo run -p jsondb_cli -- --space un2 --db _system import actors ./data_source/actors/
```

---

## 🔍 Moteur SQL & Recherche

Le CLI intègre un moteur SQL capable de filtrer et projeter les données JSON.

### `sql`

Exécute une requête SQL standard.

**Fonctionnalités supportées :**

- `SELECT` avec projection (`SELECT handle, kind`)
- `WHERE` avec opérateurs complexes (`=`, `!=`, `>`, `<`, `LIKE`, `AND`, `OR`, parenthèses)
- `ORDER BY` (Tri ascendant/descendant)

<!-- end list -->

```bash
# Exemple complexe
cargo run -p jsondb_cli -- --space un2 --db _system sql "SELECT handle, kind FROM actors WHERE kind = 'human' AND tags LIKE 'admin' ORDER BY createdAt DESC"
```

---

## 🔄 Transactions Intelligentes

### `transaction`

Exécute un lot d'opérations de manière atomique (ACID). Le moteur transactionnel est "Smart" : il sait résoudre des références métier.

**Format du fichier de transaction (`tx.json`) :**

```json
{
  "operations": [
    {
      "type": "insert",
      "collection": "actors",
      "document": {
        "handle": "new-user",
        "displayName": "Nouvel Utilisateur",
        "kind": "human"
      }
    },
    {
      "type": "update",
      "collection": "actors",
      "handle": "admin-cli", // <-- Résolution automatique par Handle !
      "document": {
        "x_active": true,
        "tags": ["verified"]
      }
    }
  ]
}
```

**Commande :**

```bash
cargo run -p jsondb_cli -- --space un2 --db _system transaction ./tx.json
```

**Points Forts :**

- **Résolution** : Pas besoin de connaître l'UUID pour faire un Update, le `handle` suffit.
- **Merge** : L'Update fusionne les champs (PATCH) au lieu d'écraser le document.
- **Sécurité** : Si une opération échoue (ex: validation schéma), **rien** n'est écrit (Rollback).

---

## ⚠️ Dépannage

**Erreur : "Variable ENV manquante"**

> Vérifiez votre fichier `.env`.

**Erreur : "Schéma introuvable"**

> Vérifiez que `create-db` a bien copié les schémas dans `data/<space>/<db>/schemas/v1/`.

**Erreur : "Missing required property" (Transaction)**

> Le document que vous essayez d'insérer ne respecte pas le schéma JSON strict (ex: champ obligatoire manquant). La transaction a été annulée par sécurité.
