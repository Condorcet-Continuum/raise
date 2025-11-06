# JSON Database Module

Base de données JSON embarquée pour GenAptitude avec :

## Fonctionnalités

- ✅ Collections avec schémas JSON Schema
- ✅ Validation automatique des documents
- ✅ Support JSON-LD pour données liées
- ✅ Indexes (B-Tree, Hash, Full-Text)
- ✅ Transactions ACID
- ✅ Migrations de schémas
- ✅ Requêtes expressives
- ✅ Cache en mémoire
- ✅ Compression automatique

## Architecture

```
json_db/
├── collections/      # Gestion des collections
├── schema/          # Validation JSON Schema
├── jsonld/          # Support JSON-LD
├── query/           # Moteur de requêtes
├── storage/         # Stockage fichier + cache
├── transactions/    # ACID + WAL
├── indexes/         # Indexation
└── migrations/      # Évolution des schémas
```

## Usage

Voir `docs/json-db.md` pour la documentation complète.
