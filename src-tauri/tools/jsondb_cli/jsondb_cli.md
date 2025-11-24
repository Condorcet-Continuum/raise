# 📚 Guide d'utilisation de la CLI `jsondb_cli`

La CLI **`jsondb_cli`** est l'outil en ligne de commande de GenAptitude pour interagir directement avec la base de données JSON. Elle permet de gérer l'environnement, les collections, les documents et d'exécuter des requêtes.

## ⚙️ Configuration et Environnement

Avant d'utiliser la CLI, assurez-vous que les variables d'environnement sont définies (via un fichier `.env` ou l'export shell) :

| Variable                  | Description                                                      |
| :------------------------ | :--------------------------------------------------------------- |
| `PATH_GENAPTITUDE_DOMAIN` | **Requis**. Chemin racine où les bases de données sont stockées. |
| `RUST_LOG`                | (Optionnel) Niveau de log (ex: `info` ou `debug`).               |

## Structure Générale

```bash
jsondb_cli [OPTIONS] <MODULE> <ACTION> [ARGUMENTS]
```

Pour un aperçu rapide dans le terminal :

```bash
jsondb_cli usage
```

---

## 🛠️ Options Globales

Ces options doivent être placées **avant** la sous-commande (`<MODULE>`).

| Option        | Description                                                                                            | Exemple                                   |
| :------------ | :----------------------------------------------------------------------------------------------------- | :---------------------------------------- |
| `--repo-root` | Spécifie explicitement la racine du dépôt (pour localiser `schemas/v1`). Par défaut : dossier courant. | `jsondb_cli --repo-root .. db create ...` |

---

## 1. Commandes de Base de Données (`db`)

Gestion de l'environnement physique et requêtes rapides.

| Action   | Description                             | Arguments                    | Exemple                                 |
| :------- | :-------------------------------------- | :--------------------------- | :-------------------------------------- |
| `create` | Crée une DB et initialise sa structure. | `<space> <db>`               | `jsondb_cli db create un2 _system`      |
| `open`   | Vérifie l'existence et l'intégrité.     | `<space> <db>`               | `jsondb_cli db open un2 _system`        |
| `drop`   | Supprime la DB.                         | `<space> <db> [--hard]`      | `jsondb_cli db drop un2 _system --hard` |
| `query`  | **Requête Ad-Hoc** sur une collection.  | `<space> <db> <coll> [OPTS]` | _(Voir détails ci-dessous)_             |

### Détail de la commande `db query`

Permet d'interroger une collection sans créer de fichier JSON de requête.

**Options :**

- `--filter-json <JSON>` : Filtre style QueryFilter (ex: `{"op":"eq",...}`).
- `--sort <field>:<asc|desc>` : Tri (répétable).
- `--limit <N>` : Limite de résultats.
- `--offset <N>` : Pagination.
- `--latest` : Raccourci pour trier par `createdAt:desc`.

**Exemple :**

```bash
jsondb_cli db query un2 _system articles \
  --filter-json '{"op":"eq","field":"status","value":"published"}' \
  --latest \
  --limit 5
```

---

## 2. Commandes de Collections (`collection`)

Gestion des collections au sein d'une DB.

| Action   | Description                           | Arguments                             | Exemple                                                                                   |
| :------- | :------------------------------------ | :------------------------------------ | :---------------------------------------------------------------------------------------- |
| `create` | Crée une collection et lie un schéma. | `<space> <db> <name> --schema <path>` | `jsondb_cli collection create un2 _system articles --schema articles/article.schema.json` |

---

## 3. Commandes de Documents (`document`)

Opérations unitaires sur les fichiers.

**Note :** `<schema>` est le chemin relatif du schéma dans `schemas/v1` (ex: `articles/article.schema.json`).

| Action   | Description                                                   | Arguments                              | Exemple                                                               |
| :------- | :------------------------------------------------------------ | :------------------------------------- | :-------------------------------------------------------------------- |
| `insert` | Insère un document (valide + x_compute). Échoue si ID existe. | `<space> <db> --schema <s> --file <f>` | `jsondb_cli document insert un2 _system --schema ... --file doc.json` |
| `upsert` | Insère ou met à jour le document.                             | `<space> <db> --schema <s> --file <f>` | `jsondb_cli document upsert un2 _system --schema ... --file doc.json` |

---

## 4. Commandes de Dataset (`dataset`)

Opérations de masse pour l'initialisation (seeding).

| Action     | Description                                                                      | Arguments                 | Exemple                                                   |
| :--------- | :------------------------------------------------------------------------------- | :------------------------ | :-------------------------------------------------------- |
| `seed-dir` | Insère tous les `.json` d'un dossier. Le nom du dossier détermine la collection. | `<space> <db> <dir_path>` | `jsondb_cli dataset seed-dir un2 _system ./data/articles` |

---

## 5. Moteur de Requêtes Avancé (`query`)

Pour les requêtes complexes définies dans un fichier séparé.

| Action      | Description                                       | Arguments             | Exemple                                                           |
| :---------- | :------------------------------------------------ | :-------------------- | :---------------------------------------------------------------- |
| `find-many` | Exécute une requête définie dans un fichier JSON. | `<space> <db> <file>` | `jsondb_cli query find-many un2 _system ./queries/my_search.json` |

**Format du fichier de requête :**

```json
{
  "collection": "articles",
  "filter": {
    "operator": "and",
    "conditions": [{ "field": "tags", "operator": "contains", "value": "rust" }]
  },
  "sort": [{ "field": "title", "order": "asc" }],
  "limit": 10
}
```

---

## 6. Commandes SQL (`sql`)

_(Expérimental / Placeholder)_

| Action | Description                                | Arguments              |
| :----- | :----------------------------------------- | :--------------------- |
| `exec` | Exécute une commande SQL (non implémenté). | `<space> <db> <query>` |
