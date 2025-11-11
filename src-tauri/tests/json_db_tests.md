# JSON DB — Guide des tests (`src-tauri/tests/json_db_tests.md`)

Ce document décrit **comment exécuter, étendre et fiabiliser** les tests du module **json_db**.
Il est conçu pour vivre **à côté des tests** (dans `src-tauri/tests`) et pointer vers la doc
fonctionnelle de référence : voir `docs/json-db.md` pour les concepts, schémas et API.

---

## 🧭 Objectifs des tests

- Vérifier l’**intégration de bout en bout** : `SchemaRegistry` → `SchemaValidator` → `x_compute` → `CRUD` fichier.
- Garantir que les **schémas JSON** (avec `$ref` & `x_compute`) pré-remplissent correctement les champs obligatoires (`$schema`, `id`, `createdAt`, `updatedAt`, etc.).
- Assurer que les **opérations collections** (`create`, `insert`, `get`, `update`, `delete`, `list`) restent **idempotentes** et **déterministes**.

---

## 📁 Fichiers de test présents (exemples)

> La liste exacte dépend de votre repo. Exemples courants déjà utilisés dans GenAptitude :

- `schema_minimal.rs` — compile un schéma, applique `x_compute`, puis **valide** un document minimal.
- `json_db_integration.rs` — **flow complet** : création DB/collection → insert with schema → lecture par `id`.

Si vous ajoutez d’autres tests (ex. `collections_crud.rs`, `validator_required.rs`…), liez-les ici pour
faciliter la maintenance.

---

## ▶️ Commandes `cargo` utiles

Exécuter **tous** les tests du crate `genaptitude` (profil par défaut) :

```bash
cargo test -p genaptitude -- --nocapture
```

Exécuter un **fichier de test** ciblé :

```bash
cargo test -p genaptitude --test schema_minimal -- --nocapture
cargo test -p genaptitude --test json_db_integration -- --nocapture
```

Exécuter un **test précis** dans un fichier :

```bash
cargo test -p genaptitude --test schema_minimal schema_instantiate_validate_minimal -- --nocapture
```

Optionnel (si vous utilisez `cargo-nextest`) :

```bash
cargo nextest run -p genaptitude
```

---

## 🧪 Patrons de tests recommandés

### 1) Test minimal “compute + validate” (extrait simplifié)

```rust
use genaptitude::json_db::schema::{SchemaRegistry, SchemaValidator};
use genaptitude::json_db::storage::{file_storage, JsonDbConfig};
use serde_json::json;
use std::path::Path;

#[test]
fn schema_instantiate_validate_minimal() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let cfg = JsonDbConfig::from_env(repo_root).expect("cfg from env");
    let (space, db) = ("un2", "_system");
    let _ = file_storage::create_db(&cfg, space, db);

    let reg = SchemaRegistry::from_db(&cfg, space, db).expect("registry");
    let root_uri = reg.uri("actors/actor.schema.json");
    let validator = SchemaValidator::compile_with_registry(&root_uri, &reg).expect("compile");

    let mut doc = json!({
        "handle":"devops-engineer",
        "displayName":"Ingénieur DevOps",
        "label":{"fr":"Ingénieur DevOps","en":"DevOps Engineer"},
        "emoji":"🛠️","kind":"human","tags":["core"]
    });

    validator.compute_then_validate(&mut doc).expect("compute+validate");
    assert_eq!(doc.get("$schema").and_then(|v| v.as_str()), Some(&root_uri));
}
```

### 2) Test d’intégration “CRUD”

```rust
use genaptitude::json_db::{collections, storage::{file_storage, JsonDbConfig}};
use serde_json::json;
use std::path::Path;

#[test]
fn insert_actor_flow() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let cfg = JsonDbConfig::from_env(repo_root).unwrap();
    let (space, db) = ("un2", "_system");
    let schema_rel = "actors/actor.schema.json";

    let _ = file_storage::create_db(&cfg, space, db);
    collections::create_collection(&cfg, space, db, "actors").unwrap();

    let doc = json!({
      "handle":"devops-engineer",
      "displayName":"Ingénieur DevOps",
      "label":{"fr":"Ingénieur DevOps","en":"DevOps Engineer"},
      "emoji":"🛠️","kind":"human","tags":["core"]
    });
    let stored = collections::insert_with_schema(&cfg, space, db, schema_rel, doc).unwrap();

    let id = stored.get("id").and_then(|v| v.as_str()).unwrap();
    let loaded = collections::get(&cfg, space, db, "actors", id).unwrap();
    assert_eq!(loaded.get("id"), stored.get("id"));
}
```

---

## 🧱 Données, chemins et isolement

- **Espace/DB par défaut** dans les tests : `un2/_system`.  
  Le helper `file_storage::create_db` est **idempotent** ; il garantit la présence de l’arborescence :

  ```text
  <db_root>/un2/_system/
  ├─ schemas/v1/...
  └─ collections/actors/...
  ```

- **Isolement** : si vous ajoutez des tests en parallèle, préférez des **espaces temporisés**  
  (ex. `un2_test_<timestamp>`) ou des **répertoires temporaires** (via `tempfile`) pour éviter
  les collisions d’écriture entre tests.

- **Nettoyage** : évitez de supprimer les dossiers partagés par d’autres tests. Si besoin, isolez
  le jeu d’essai dans un espace dédié pour pouvoir le supprimer à la fin du test.

---

## ✅ Bonnes pratiques (tests json_db)

1. **Deux niveaux de doc** :
   - `docs/json-db.md` = documentation **fonctionnelle & API** (source canonique).
   - `src-tauri/tests/json_db_tests.md` = **guide pratique des tests** (rapide, ciblé).
2. **Ne dupliquez pas** de longues sections entre les deux ; **liez** vers `docs/json-db.md`.
3. **AAA** (Arrange–Act–Assert) dans chaque test ; messages `expect()` explicites.
4. **Déterminisme** : évitez d’asserter des valeurs horodatées exactes (`createdAt/updatedAt`).
   - Vérifiez seulement la **présence** ou le **format** (regex), ou **moquez la clock** si nécessaire.
5. **x_compute d’abord, validation ensuite** : utilisez toujours `compute_then_validate`.
6. **Paths & schémas** : préférez l’URI logique (`db://.../schemas/v1/...`) via `reg.uri(rel)`.
7. **Pas d’I/O inutiles** : groupez les opérations FS (création de DB, chargement de registre).
8. **Snapshots** (optionnel) : pour des objets volumineux, `insta` est utile, mais gardez les snapshots stables.
9. **Nommage des tests** : explicite et par cas d’usage (`insert_actor_flow`, `validate_missing_required`, etc.).
10. **Vitesse** : gardez les tests unitaires très rapides ; réservez les scénarios lourds pour l’intégration.

---

## 🔗 Voir aussi

- **Doc fonctionnelle** : `docs/json-db.md`
- **Modules principaux** :
  - `src-tauri/src/json_db/schema/` (schemas, compute, validator)
  - `src-tauri/src/json_db/collections/` (CRUD fichiers & manager)
  - `src-tauri/src/json_db/storage/` (chemins & création DB)

---

## 🧩 Modèle de nouveau test

Copiez-collez ce squelette pour un nouveau fichier dans `src-tauri/tests/` :

```rust
use genaptitude::json_db::{
    collections,
    schema::{SchemaRegistry, SchemaValidator},
    storage::{file_storage, JsonDbConfig},
};
use serde_json::json;
use std::path::Path;

#[test]
fn my_new_scenario() {
    // Arrange
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let cfg = JsonDbConfig::from_env(repo_root).unwrap();
    let (space, db) = ("un2", "_system");
    let _ = file_storage::create_db(&cfg, space, db);

    // Act
    let reg = SchemaRegistry::from_db(&cfg, space, db).unwrap();
    let root_uri = reg.uri("actors/actor.schema.json");
    let validator = SchemaValidator::compile_with_registry(&root_uri, &reg).unwrap();

    let mut doc = json!({
        "handle":"new-handle",
        "displayName":"Label",
        "label":{"fr":"Label","en":"Label"},
        "emoji":"✨","kind":"human","tags":["core"]
    });
    validator.compute_then_validate(&mut doc).unwrap();

    // Assert
    assert_eq!(doc.get("$schema").and_then(|v| v.as_str()), Some(&root_uri));
}
```

---

> _Dernier conseil_: si vous hésitez entre écrire ce guide ici **ou** dans `docs/`, gardez la
> **doc longue** dans `docs/` et utilisez **ce fichier à la racine des tests** pour tout ce qui
> concerne **l’exécution** et les **astuces pratiques**. Cela maintient la doc claire et
> **découvrable là où on en a besoin**.
