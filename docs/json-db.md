# GenAptitude — JSON DB (Guide)

> **TL;DR**  
> - Schémas chargés depuis `db://{space}/{db}/schemas/v1/**/*.json` via **SchemaRegistry** (offline only).  
> - `$ref` résolus **strictement** dans le registre (pas d’accès réseau).  
> - `x_compute` (engine `plan/v1`) pré-remplit automatiquement `id`, `createdAt`, `updatedAt`, `$schema`, etc.  
> - La **collection** est déduite du chemin du schéma (ex: `actors/actor.schema.json` → `actors`).  
> - Persistance fichier : `…/collections/<collection>/<id>.json`.  
> - Ordre d’exécution : **compute → validate → persist**.

---

## 1) Arborescence (DB)

```
<repo-root>/
 └─ <space>/<db>/
     ├─ schemas/
     │   └─ v1/
     │       ├─ common/types/primitive-types.schema.json
     │       └─ actors/actor.schema.json
     └─ collections/
         └─ actors/
             └─ <id>.json
```

- URI logique schéma : `db://<space>/<db>/schemas/v1/<relpath>.json`  
- **Collection** = premier segment de `<relpath>` (ex: `actors/…` → `actors`).  
- Les `$ref` peuvent pointer vers :
  - le **même document** (`"#/..."`)
  - ou un **autre schéma du registre** (`"common/types/primitive-types.schema.json#/$defs/_id"`)

---

## 2) `x_compute` — Engine `plan/v1`

### Opérations supportées
- **Générateurs** :  
  - `uuid_v4` → UUID v4  
  - `now_rfc3339` → horodatage RFC3339
- **Arithmétique** : `add`, `sub`, `mul`, `div`, `round(scale)`  
- **Agrégat** : `sum` avec :  
  - `from` (JSON Pointer vers un tableau),  
  - `path` (clé ou JSON Pointer dans chaque élément),  
  - `where` (filtre simple: `{ ptr, op, value }`)
- **Logique/Comparateurs** : `and`, `or`, `not`, `lt`, `le`, `gt`, `ge`, `eq`, `ne`
- **Pointeurs JSON** : `{"ptr":"#/a/b"}` avec **scope** `self` / `root`, support de `../`  
  - `scope: "self"` : tente d’abord relatif à l’objet courant (fallback racine si pas de `../`, sauf `strict_ptr=true`)

### Stratégie `update`
- `always` : réécrit systématiquement  
- `if_missing` : écrit si `Null`/absent (+ tolérance placeholders `"00000000-..."`, `"1970-01-01T00:00:00Z"` si l’opération s’y prête)  
- `if_null` : écrit si `Null` uniquement

### Interaction avec `required`
- À l’**insert**, on **compute puis on valide** : si un champ `required` provient d’un `$ref` porteur de `x_compute`, il est rempli avant la validation.

### `$schema`
- Injecté **automatiquement** si absent, avec la valeur de l’URI logique du schéma racine, ex:  
  `db://un2/_system/schemas/v1/actors/actor.schema.json`

---

## 3) API Rust (résumé)

### Niveau bas (free functions) — `json_db::collections`
- **Collections** :  
  `create_collection(cfg, space, db, name)`, `drop_collection(...)`
- **Insert/Update** :  
  `insert_with_schema(cfg, space, db, schema_rel, doc)`  
  `update_with_schema(cfg, space, db, schema_rel, doc)`  
  `insert_raw(cfg, space, db, collection, &doc)`  
  `update_raw(cfg, space, db, collection, &doc)`
- **Lecture / Suppression / Listes** :  
  `get(cfg, space, db, collection, id)`  
  `delete(cfg, space, db, collection, id)`  
  `list_ids(cfg, space, db, collection)`  
  `list_all(cfg, space, db, collection)`

### Niveau instance — `json_db::collections::manager::CollectionsManager`
- Cache un **SchemaRegistry** (lazy) et expose des méthodes CRUD cohérentes :  
  `insert_with_schema`, `update_with_schema`, `upsert_with_schema`, `get`, `delete`, `list_ids`, `list_all`, etc.

---

## 4) Exemples

### Insert + compute + validate (free functions)

```rust
use genaptitude::json_db::{
  collections,
  storage::{file_storage, JsonDbConfig},
};
use serde_json::json;
use std::path::Path;

let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
let cfg = JsonDbConfig::from_env(repo_root)?;
let (space, db) = ("un2", "_system");

// idempotent
file_storage::create_db(&cfg, space, db)?;
collections::create_collection(&cfg, space, db, "actors")?;

let schema_rel = "actors/actor.schema.json";
let doc = json!({
  "handle":"devops-engineer",
  "displayName":"Ingénieur DevOps",
  "label":{"fr":"Ingénieur DevOps","en":"DevOps Engineer"},
  "emoji":"🛠️","kind":"human","tags":["core"]
});

let stored = collections::insert_with_schema(&cfg, space, db, schema_rel, doc)?;
assert!(stored.get("$schema").is_some());
assert!(stored.get("id").is_some());
```

### Manager orienté instance

```rust
use genaptitude::json_db::collections::manager::CollectionsManager;
use serde_json::json;

let mgr = CollectionsManager::new(&cfg, space, db);
let stored = mgr.insert_with_schema("actors/actor.schema.json", json!({
  "handle":"sre-engineer",
  "displayName":"Ingénieur SRE",
  "label":{"fr":"Ingénieur SRE","en":"SRE Engineer"},
  "emoji":"🛠️","kind":"human","tags":["core"]
}))?;

let id = stored["id"].as_str().unwrap();
let got = mgr.get("actors", id)?;
assert_eq!(got["id"], stored["id"]);
```

---

## 5) Tests (`src-tauri/tests`)

### A) `schema_minimal.rs`
Vérifie :
- Préremplissage : `$schema`, `id`, `createdAt`, `updatedAt`
- Validité du document après compute

Exécution :
```bash
cargo test -p genaptitude --test schema_minimal -- --nocapture
```

### B) `json_db_integration.rs`
CRUD bout-en-bout : create DB, create collection, `insert_with_schema`, `get` par `id`.

Exécution :
```bash
cargo test -p genaptitude --test json_db_integration -- --nocapture
```

---

## 6) Erreurs courantes & diagnostic

- **Missing required property**
  - Le champ est réellement absent **et** non calculable.  
  - Action : vérifier le schéma de la propriété (présence d’un `$ref` vers un bloc `x_compute` ou d’un `default/const/enum`), ou renseigner la valeur côté appelant.

- **$ref not found in registry**
  - Le chemin référencé n’existe pas dans `schemas/v1`, ou l’URI n’est pas relative à la base.  
  - Action : corriger la cible (`db://…/schemas/v1/<rel>#/ptr`) ou le `relpath` utilisé.

- **Conflit d’ID à l’insert**
  - Le fichier `<id>.json` existe déjà.  
  - Action : utiliser `update_with_schema` ou une logique `upsert_with_schema`.

- **Type mismatch / enum violation**
  - La valeur calculée n’est pas du type attendu, ou n’appartient pas à l’`enum`.  
  - Action : corriger le plan `x_compute` ou ajuster le schéma.

Astuce debug : journaliser le **document après compute** (avant validate) pour comprendre ce qui a été injecté :
```rust
let mut doc = input.clone();
validator.compute_then_validate(&mut doc)?;
eprintln!("doc après compute: {}", doc);
```

---

## 7) Roadmap

- Validation enrichie : `pattern`, `format` (uuid/date-time), `oneOf/anyOf/allOf` complets.  
- Hooks (pre/post compute), stratégies de merge avancées.  
- Index & requêtage simple (filtres/tri côté filesystem).  
- Outillage CLI (`jsondb_cli`) pour introspection et migrations de schémas.

---

## 8) Bonnes pratiques

- Préférer des schémas **petits et composables** via `$defs` + `$ref`.  
- Utiliser `update: "always"` avec parcimonie (ex: `updatedAt`).  
- Pour référencer des “frères”, utiliser `scope: "self"` + `../`.  
- Garder les identifiants (`id`) **stables** entre updates ; réserver l’insert à la création.
