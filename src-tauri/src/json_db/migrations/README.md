# Module Migrations (JSON-DB)

Ce module fournit un système robuste de **gestion de versions de schéma** pour la base de données JSON (NoSQL) de GenAptitude.

Bien que JSON-DB soit "Schemaless" (sans schéma rigide) par nature, l'application a besoin de garanties sur la structure des données pour fonctionner correctement. Ce module permet de faire évoluer la structure des données existantes (ajout de champs, renommage, indexation) de manière ordonnée et traçable.

## 🏗️ Architecture

Le module est composé de trois fichiers principaux :

1.  **`mod.rs`** : Définit les structures de données (`Migration`, `MigrationStep`) qui décrivent une évolution de la base.
2.  **`version.rs`** : Gère le parsing et la comparaison des versions selon le **Semantic Versioning** (ex: `1.0.0` < `1.1.0`).
3.  **`migrator.rs`** : Le moteur d'exécution. Il compare les migrations déclarées dans le code avec l'historique de la base, applique les changements et met à jour le registre.

## 🚀 Fonctionnalités

### Opérations Supportées (`MigrationStep`)

Le système supporte les opérations atomiques suivantes :

| Opération              | Description                                                                       | Impact Performance     |
| :--------------------- | :-------------------------------------------------------------------------------- | :--------------------- |
| **`CreateCollection`** | Crée une nouvelle collection et son fichier `_meta.json` (avec schéma optionnel). | 🟢 Faible              |
| **`DropCollection`**   | Supprime une collection entière.                                                  | 🟢 Faible              |
| **`CreateIndex`**      | Ajoute un index (ex: BTree) sur un champ spécifique.                              | 🟡 Moyen               |
| **`DropIndex`**        | Supprime un index existant.                                                       | 🟢 Faible              |
| **`AddField`**         | Ajoute un champ avec une valeur par défaut à **tous** les documents.              | 🔴 Fort (Scan complet) |
| **`RemoveField`**      | Supprime un champ de **tous** les documents.                                      | 🔴 Fort (Scan complet) |
| **`RenameField`**      | Renomme une clé dans **tous** les documents (ex: `cost` -> `price`).              | 🔴 Fort (Scan complet) |

### Gestion de l'État (`_migrations`)

Le module utilise une collection système privée nommée **`_migrations`** pour stocker l'historique.
À chaque démarrage, le `Migrator` :

1.  Vérifie l'existence de la collection `_migrations`.
2.  Lit les migrations déjà appliquées (Idempotence).
3.  Trie les nouvelles migrations par version (SemVer).
4.  Exécute uniquement celles qui manquent.

## 🛠️ Exemple d'Utilisation

Voici comment déclarer et exécuter des migrations au démarrage de l'application (dans `main.rs` ou un module d'initialisation) :

```rust
use crate::json_db::migrations::{Migration, MigrationStep, Migrator};
use serde_json::json;

pub fn init_database_migrations(storage: &StorageEngine, space: &str, db: &str) -> Result<()> {
    let migrator = Migrator::new(storage, space, db);

    let migrations = vec![
        // V1 : Initialisation
        Migration {
            id: "m_init_users".to_string(),
            version: "1.0.0".to_string(),
            description: "Création table utilisateurs".to_string(),
            up: vec![
                MigrationStep::CreateCollection {
                    name: "users".to_string(),
                    schema: json!(null)
                }
            ],
            down: vec![], // Rollback non implémenté pour l'instant
            applied_at: None,
        },
        // V2 : Évolution du schéma
        Migration {
            id: "m_add_active_flag".to_string(),
            version: "1.1.0".to_string(),
            description: "Ajout flag actif par défaut".to_string(),
            up: vec![
                MigrationStep::AddField {
                    collection: "users".to_string(),
                    field: "is_active".to_string(),
                    default: Some(json!(true))
                }
            ],
            down: vec![],
            applied_at: None,
        }
    ];

    // Exécution automatique
    migrator.run_migrations(migrations)?;
    Ok(())
}
```

````

## ✅ Tests et Validation

Ce module est couvert par des tests unitaires validant :

- Le parsing des versions (`1.2.3`).
- L'ordre d'application des migrations.
- La modification réelle des fichiers JSON sur le disque (Renommage, Ajout).
- L'idempotence (ne pas ré-appliquer une migration déjà faite).

Pour lancer les tests spécifiques à ce module :

```bash
cargo test --manifest-path src-tauri/Cargo.toml json_db::migrations

```

**Résultat attendu :**

```text
running 5 tests
test json_db::migrations::migrator::tests::test_migration_lifecycle ... ok
test json_db::migrations::migrator::tests::test_rename_field ... ok
test json_db::migrations::version::tests::test_version_ordering ... ok
test json_db::migrations::version::tests::test_version_parsing ... ok
test json_db::migrations::version::tests::test_version_sorting_list ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 101 filtered out

```

## ⚠️ Notes Techniques

1. **Transformations Lourdes** : Les opérations `AddField`, `RemoveField` et `RenameField` impliquent l'ouverture, la modification et la réécriture de **chaque fichier JSON** de la collection cible. À utiliser avec parcimonie sur les très grosses collections.
2. **Schémas** : Lors d'une migration, si un schéma JSON (`$schema`) est actif sur la collection, `update_document` tentera de valider le document. Si la migration rend le document invalide temporairement, assurez-vous de mettre à jour le schéma AVANT ou DANS la même migration.

```

```

````
