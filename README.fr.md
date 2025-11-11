<p align="center"><img src="src/assets/images/logo-white.svg" alt="GenAptitude" width="180"></p>

# GenAptitude · Usine de Cas d'Usage IA Orientée Poste de Travail

Transformez vos tâches métier répétitives en assistants **locaux, auditables et explicables**.  
Ce monorepo contient l'**application de bureau (Tauri v2 + Rust)**, le **frontend (Vite + React + TypeScript)**, une démonstration **Rust→WASM**, et un moteur de **base de données JSON** avec calcul et validation pilotés par schéma.

## Pourquoi MBAIE (Ingénierie Neuro-Symbolique IA Basée sur les Modèles) ?

GenAptitude adopte **MBAIE** pour combiner les forces de l'IA **neuronale** (LLMs, embeddings, recherche vectorielle) et **symbolique** (ontologies, moteurs de règles, solveurs déterministes) dans une ossature **basée sur les modèles**. Les connaissances métier sont modélisées explicitement (approche Arcadia/Capella ; schémas JSON/JSON-LD, événements typés, contrats), puis exécutées par un pipeline où :

1) La récupération et les LLMs génèrent des hypothèses ;  
2) Les **règles/contraintes** vérifient la conformité et comblent les lacunes ;  
3) Les **explications** et **preuves** (sources, traces de règles) sont attachées à chaque sortie ;  
4) Les artefacts sont **versionnés et auditables** de bout en bout.

Cela garantit **cohérence, contrôlabilité et confiance**, tout en restant **orienté poste de travail** (confidentialité, coût, énergie) et prêt pour l'**amélioration continue** (affinage LoRA/QLoRA contre des suites de tests basées sur les modèles).

---

## ✨ Points Forts

- **Orienté poste de travail et souverain** : s'exécute localement ; pas de dépendance cloud
- **Bureau Tauri v2** : empreinte réduite, packaging natif
- **Frontend** : Vite + React (TS). La racine Vite est `src/` ; les ressources statiques dans `public/`
- **Démo WASM** : `ga_wasm.wasm` servi depuis `public/wasm/`
- **Base de données JSON** : registre de schémas, résolution `$ref`, `x_compute` (plan/v1), validation
- **CI (GitLab)** : construit les artefacts web, compile WASM, bundle les installateurs Linux

---

## Structure du Dépôt

```text
.
├─ src/                         # Racine Vite (frontend)
│  ├─ index.html
│  ├─ main.tsx / App.tsx
│  └─ pages/
│     └─ dark-mode-demo.html
├─ public/                      # Copié tel quel → dist/
│  └─ wasm/ga_wasm.wasm
├─ dist/                        # Sortie de build frontend (généré)
├─ src-tauri/                   # Tauri v2 (Rust)
│  ├─ src/
│  │  ├─ main.rs                # Commandes Tauri + bootstrap
│  │  ├─ commands/              # ex. json_db_commands.rs
│  │  └─ json_db/
│  │     ├─ collections/        # collection FS + facade manager
│  │     ├─ schema/             # registre, validateur, calcul (x_compute)
│  │     └─ storage/            # JsonDbConfig + assistants FS
│  └─ tauri.conf.json           # "frontendDist": "../dist"
├─ src-wasm/                    # Crate Rust → WASM (wasip1/unknown)
├─ docs/
│  ├─ json-db.md                # Documentation approfondie de la base JSON
│  └─ commands/json_db_commands.md
└─ .gitlab-ci.yml               # Pipeline GitLab (web, wasm, tauri bundle)
```

---

## Prérequis

- **Node 20+** et un gestionnaire de paquets (npm / pnpm / yarn)
- **Rust 1.88+** avec `rustup`
- Cibles WASM :
  ```bash
  rustup target add wasm32-unknown-unknown wasm32-wasip1
  ```
- (Optionnel pour packaging local) Bibliothèques de développement WebKitGTK/JavaScriptCore/GTK (CI bundle déjà les installateurs).

---

## Démarrage Rapide

### Frontend (développement navigateur)
```bash
npm install
npm run dev
# Ouvrir http://localhost:1420
```

### Bureau (développement Tauri)
Exécute Vite pour vous via `beforeDevCommand` :
```bash
cargo tauri dev
```

### Build de Production
```bash
# 1) Construire le frontend → ./dist
npm run build

# 2) Bundler l'application de bureau → ./target/release/bundle/**
cargo tauri build
# Produit AppImage, .deb, .rpm dans target/release/bundle/
```

---

## Base de Données JSON — Tour en 60 secondes

- Les schémas se trouvent sous : `db://{espace}/{db}/schemas/v1/**`
- Le **registre** charge tous les schémas ; le **validateur** effectue `x_compute` puis `validate` (requis, types, gestion defaults/const/enum).
- Les collections sont mappées depuis les chemins de schémas (ex. `actors/actor.schema.json` → collection `actors/`).

Insertion minimale (Rust) :
```rust
use serde_json::json;
use genaptitude::json_db::collections::insert_with_schema;

let stored = insert_with_schema(
  &cfg, "un2", "_system", "actors/actor.schema.json",
  json!({
    "handle":"devops-engineer",
    "displayName":"DevOps Engineer",
    "label":{"fr":"Ingénieur DevOps","en":"DevOps Engineer"},
    "emoji":"🛠️","kind":"human","tags":["core"]
  })
)?;
// stored contient maintenant : $schema, id (uuid), createdAt, updatedAt
```

▶ Voir **`docs/json-db.md`** pour le guide complet (règles de schéma, plan de calcul, pointeurs, tests).

---

## Tests

Exécutez les tests unitaires/intégration depuis le crate Tauri :
```bash
# Tous les tests
cargo test -p genaptitude -- --nocapture

# Fichier de test spécifique
cargo test -p genaptitude --test schema_minimal -- --nocapture

# Exemple de suite d'intégration
cargo test -p genaptitude --test json_db_integration -- --nocapture
```

Un guide rapide est disponible dans **`src-tauri/tests/json_db_tests.md`**.

---

## CI/CD (GitLab)

Étapes : **lint → build → test → bundle**.

- **web:build** — Build Vite ; publie `dist/` comme artefact.  
- **wasm:build** — construit `src-wasm` pour `wasm32-unknown-unknown` et `wasm32-wasip1`.  
- **rust:test** — exécute les tests pour les crates `src-wasm` et Tauri.  
- **tauri:bundle** — Dépendances Debian 12 (`libwebkit2gtk-4.1-dev`, `libjavascriptcoregtk-4.1-dev`, `libsoup-3.0-dev`), puis `cargo tauri build` → AppImage/.deb/.rpm.

---

## Dépannage

- **Reconstruction infinie dans `cargo tauri dev`** : n'écrivez pas de fichiers sous `src-tauri/` depuis le frontend. Utilisez le répertoire user-data de l'OS.
- **Écran blanc dans le bureau** : assurez-vous que `npm run build` a été exécuté et que `tauri.conf.json` utilise `"frontendDist": "../dist"`.
- **WASM 404** : assurez-vous que `public/wasm/ga_wasm.wasm` existe avant le build ; il apparaîtra dans `dist/wasm/`.
- **Port en cours d'utilisation** : changez le `server.port` de Vite (et `devUrl` dans `tauri.conf.json`) ou arrêtez le serveur de développement précédent.

---

## Contribution

Les PRs sont les bienvenues. Veuillez garder les modifications petites, testées et documentées. Envisagez d'ajouter une entrée à un futur `CHANGELOG.md`.

## Licence

À déterminer.

## Contact

**GenAptitude — Usine de Cas d'Usage IA Orientée Poste de Travail**  
Contact : **zair@bezghiche.com**
