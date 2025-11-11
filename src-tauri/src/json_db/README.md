# JSON Database Module

Base de données JSON embarquée pour **GenAptitude**. Ce module fournit un stockage local en fichiers JSON, validés par des schémas et enrichis automatiquement via un moteur `x_compute` (sans dépendance externe de validation).

> 📚 La documentation détaillée (concepts, conventions, exemples complets) est dans **[docs/json-db.md](../../../../docs/json-db.md)**.

---

## État des fonctionnalités

### ✅ Implémenté (MVP)
- **Collections** sur FS avec conventions de nommage (voir *Schéma → Collection*).
- **Registre de schémas** chargé depuis `db://{space}/{db}/schemas/v1/**`. Résolution stricte des `$ref` internes.
- **Validation minimale** (subset JSON Schema) : `type`, `required`, `properties`, `additionalProperties`, `enum`, `minLength`, `items`, `minItems`.
- **`x_compute` plan/v1** (auto-remplissage et calculs) :  
  - Générateurs : `uuid_v4`, `now_rfc3339`  
  - Arithmétique : `add`, `sub`, `mul`, `div`, `round(scale)`  
  - Agrégats : `sum(from, path, where?)`  
  - Logique : `and`, `or`, `not`, comparateurs `lt/le/gt/ge/eq/ne`, `cond { if/then/else }`  
  - Pointeurs JSON tolérants `ptr:"#/..."` avec portée `root/self` et `../`
- **Auto-remplissage** des champs de base si absents (via `x_compute` des schémas communs) :  
  `$schema`, `id`, `createdAt`, `updatedAt`.
- **API de haut niveau** via `CollectionsManager` (CRUD + insert/update avec schéma).  
- **Tests d’intégration** (voir `src-tauri/tests`).

### 🛣️ Roadmap (non implémenté dans ce repo à date)
- JSON-LD natif.
- Moteur de requêtes expressives.
- Indexes (B-Tree / Hash / Full-Text).
- Transactions (ACID + WAL).
- Cache mémoire structuré.
- Compression transparente.
- Migrations de schémas versionnées.

> ℹ️ Le README reflète l’état **réel du code**. Les éléments listés en “Roadmap” sont des objectifs de design, pas encore disponibles.

---

## Architecture (répertoire `src-tauri/src/json_db/`)

```
json_db/
├── collections/
│   ├── mod.rs             # Façade module + helpers
│   ├── collection.rs      # CRUD bas-niveau (FS)
│   └── manager.rs         # CollectionsManager (CRUD haut-niveau + schémas)
├── schema/
│   ├── registry.rs        # Chargement + résolution stricte des schémas ($ref, URI db://)
│   ├── validator.rs       # Validation minimale + pipeline compute→validate
│   ├── compute.rs         # Moteur x_compute (plan/v1)
│   └── validator_helpers.rs # Petits utilitaires de validation
└── storage/
    ├── mod.rs             # Config + helpers de chemins
    └── file_storage.rs    # I/O fichiers (create_db, read/write doc, etc.)
```

### Layout sur disque

```
{REPO_ROOT}/genaptitude_domain/{space}/{db}/
├── schemas/v1/...
└── collections/
    └── {collection}/
        └── {id}.json
```

- **URI logique** d’un schéma : `db://{space}/{db}/schemas/v1/{relpath}.json`
- **Règle “Schéma → Collection”** : on dérive le nom de collection depuis le chemin du schéma.  
  Ex. `actors/actor.schema.json` → collection **`actors`**.

---

## Conventions de schéma

- **`$schema`** : auto-inséré si manquant (URI logique complète du schéma courant).
- **Champs communs** via defs/références (exemples usuels) :  
  - `id`: `x_compute: { update: "if_missing", plan: { op: "uuid_v4" } }`  
  - `createdAt` / `updatedAt`: `x_compute: { plan: { op: "now_rfc3339" } }` (avec règles d’update).
- **`x_compute`** :
  - Portée `scope`: `root` ou `self`, support de pointeurs `#/a/b`, de `../` et fallback root (configurable).
  - Itération multi-passes jusqu’à convergence (4 par défaut).

Voir les schémas d’exemple dans `un2/_system/schemas/v1/**` et la doc *x_compute* dans **docs/json-db.md**.

---

## API principale

### Types clés
- `JsonDbConfig` : configuration (racines, chemins, env).
- `SchemaRegistry` : registre des schémas chargés (résolution `$ref` strictement locale).
- `SchemaValidator` : `compute_then_validate(&mut doc)` + `validate(&doc)`.
- `CollectionsManager` : façade CRUD instance (space/db).

### Extraits d’usage

**Insertion avec schéma (compute → validate → persist)**

```rust
use genaptitude::json_db::collections::manager::CollectionsManager;
use genaptitude::json_db::storage::JsonDbConfig;
use serde_json::json;
use std::path::Path;

let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
let cfg = JsonDbConfig::from_env(repo_root)?;
let mgr = CollectionsManager::new(&cfg, "un2", "_system");

let doc = json!({
  "handle": "devops-engineer",
  "displayName": "Ingénieur DevOps",
  "label": {"fr":"Ingénieur DevOps","en":"DevOps Engineer"},
  "emoji":"🛠️", "kind":"human", "tags":["core"]
});

let stored = mgr.insert_with_schema("actors/actor.schema.json", doc)?;
// -> remplit: $schema, id, createdAt, updatedAt si manquants
```

**Lecture / mise à jour / suppression**

```rust
// get
let id = stored.get("id").and_then(|v| v.as_str()).unwrap();
let loaded = mgr.get("actors", id)?;

// update (recompute + validate + persist)
let mut edited = loaded.clone();
edited.as_object_mut().unwrap().insert("emoji".into(), "🧰".into());
let updated = mgr.update_with_schema("actors/actor.schema.json", edited)?;

// delete
mgr.delete("actors", id)?;
```

**Tests**  
- Unitaires simples : `cargo test -p genaptitude --test schema_minimal -- --nocapture`  
- Intégration JSON DB : `cargo test -p genaptitude --test json_db_integration -- --nocapture`

---

## Lien avec la documentation

- Guide complet : **`docs/json-db.md`**  
  → Concepts, conventions, exemples de schémas, pipeline compute→validate, cas d’usage.

---

## Limitations / Design

- Pas de librairie externe de validation JSON Schema : implémentation ciblée pour nos besoins.  
- `$ref` **strictement** résolus depuis le registre local (pas de fetch externe).  
- `x_compute` est *idempotent* et conçu pour converger en quelques passes.

Pour toute évolution (indexes, transactions, JSON-LD…), ouvrir une *issue* avec cas d’usage et contraintes de performance/souveraineté.
