# GenAptitude — Technical Architecture

**Version :** 1.0 · **Date :** 2025-11-08 · **Auteur :** GenAptitude  
**Slogan :** _From Business Needs to Running Code_

> Architecture technique détaillée pour le MVP **workstation-first** : Rust/Tauri/WASM (UI IA-Native), RAG (Qdrant + LeanStore), **LLM local** (llama.cpp GGUF), **MBAIE** (neuro-symbolique), traçabilité (Hyperledger Fabric), observabilité (Prometheus/Loki/OpenTelemetry). Distinction **Software / System / Hardware**.

---

## 1. Objectifs techniques

- **Souveraineté & confidentialité** : exécution locale, données en clair uniquement si nécessaire, chemins OS “app data”.
- **Frugalité** : _retrieval-first_, quantification 4–8 bits, CPU-first avec option GPU.
- **Explicabilité & traçabilité** : _no source, no claim_, règles vérifiables, hash on-chain.
- **Observabilité** : métriques/logs/traces standard OTel.
- **Reproductibilité** : versions figées, build déterministe CI, SBOM.

---

## 2. Vue d’ensemble (composants)

```mermaid
graph TD
  subgraph Desktop[Workstation (Tauri app)]
    UI[Vite/React (TS)] -->|invoke| Tauri[Tauri v2 (Rust)]
    Tauri --> Orchestrator[Agent Orchestrator (Rust)]
    Orchestrator --> WASM[WASM Sandbox (WASI)]
    Orchestrator --> RAG[(Qdrant + LeanStore)]
    Orchestrator --> Rules[Règles: Drools/OpenFisca Adapters]
    Orchestrator --> Ledger[(Hyperledger Fabric Client)]
    Orchestrator --> Obs[(OTel Exporter)]
  end
  RAG -. embeddings .- LLM[(llama.cpp Runtime)]
```

**Flux** : UI (intentions) → Orchestrator → RAG + LLM → Validation **règles** → HITL (si besoin) → **Evidence** → Publication.

---

## 3. Pile technologique

### 3.1 Software

- **Langages** : Rust (core, Tauri, WASM), TypeScript (UI), JSON/JSON-LD (ontologies).
- **Frontend** : Vite + React, Router Hash, alias `@` → `src/*`.
- **Desktop** : Tauri v2, `tauri.conf.json` → `frontendDist: "../dist"`, `beforeDevCommand: "npm run dev"`.
- **LLM local** : llama.cpp (GGUF) — chargement **CPU** par défaut ; option **GPU** (CUDA/Metal/Vulkan).
- **RAG** : Qdrant (vecteurs) + **LeanStore** (métadonnées/relations).
- **Règles** : adaptateurs vers **Drools** (business rules) et **OpenFisca** (moteur calcul).
- **Ledger** : client Hyperledger Fabric (soumission hash + métadonnées).
- **Observabilité** : OTel (metrics/logs/traces) → Prometheus/Loki (via agent local).

### 3.2 System

- **OS cible** : Ubuntu 24.04+ (Linux desktop).
- **Dépendances build Tauri** (Linux) : WebKitGTK/JavaScriptCore/libsoup (4.1), GTK3, pkg-config, build-essential.
- **CI GitLab** : image Debian 12 (bookworm) + **backports** pour 4.1 ; pipeline `lint → build → test → bundle`.
- **Stockage local** : `{app_data_dir}/schemas`, `{app_data_dir}/evidence`, `{app_log_dir}`.
- **Sécurité** : signatures binaires, contrôle CSP (Tauri), allowlist API.

### 3.3 Hardware

- **Minimum** : CPU 4c/8t, RAM 16 Go, SSD NVMe.
- **Recommandé** : CPU 8c/16t, RAM 32 Go. **Option** GPU (NVIDIA, AMD) pour modèles plus lourds.
- **Edge** : Odroid H4+/NUC (option), stockage chiffré.

---

## 4. Modules & Interfaces

### 4.1 UI (src/)

- **Entrée** : `src/index.html`, `main.tsx`, `App.tsx`; pages statiques sous `src/pages/…`.
- **Services** : `src/services/json-db/*` (ex. `SchemaService`).
- **Exemple lien statique** : `/pages/dark-mode-demo.html`.

### 4.2 Tauri Commands (src-tauri/src/main.rs)

```rust
#[command] fn register_schema(app: AppHandle, schema_id: String, schema_json: String) -> Result<(), String>;
#[command] fn get_schema(app: AppHandle, schema_id: String) -> Result<String, String>;
```

- Persistance sous `{app_data_dir}/schemas/{schema_id}.json`.
- Ajouter ensuite : `run_plan`, `get_evidence`, `rag_search`, `validate_rules`.

### 4.3 WASM Sandbox

- **Cibles** : `wasm32-wasip1` (sandbox règles/outils), `wasm32-unknown-unknown` (pur compute).
- **Contrats** (ex.) :

```rust
// export "ga_run" (WASI)
#[no_mangle]
pub extern "C" fn ga_run(ptr: *const u8, len: usize) -> i32 { /* ... */ }
```

### 4.4 RAG Adapter

- **Embeddings** : locaux (all-MiniLM / E5-small) → ajustable.
- **Index** : Qdrant (HNSW) ; métadonnées normalisées (_doc_id, chunk_id, source_uri, hash, tags_).
- **API** : `search(query, k, filters) -> { passages, citations }`.

### 4.5 Rules Adapter

- **Drools** : règles `.drl` versionnées ; appel via service (HTTP/gRPC) ou exécution locale si embarquée.
- **OpenFisca** : API calcul (paramètres → résultats + trace).
- **Contrat** : `validate(draft, context) -> { verdict, deltas, rules_fired[] }`.

### 4.6 Evidence / Ledger

- **Hash** : SHA-256 de l’artefact + métadonnées (source set, horodatage, version).
- **Fabric** : `submitEvidence(hash, meta)` ; lecture `getEvidence(id)`.

---

## 5. Données & Config

### 5.1 Répertoires

```
{app_data_dir}/schemas/        # schémas métiers (JSON/JSON-LD)
{app_data_dir}/evidence/       # journaux, preuves, artefacts signés
{app_log_dir}/                 # logs OTel (si file-based)
public/wasm/ga_wasm.wasm       # artefacts servis tels quels
```

### 5.2 Nommage & version

- `$id` stable (`urn:ga:schema:<name>:v<major>`), `@context` explicite.
- Dossiers `schemas/core/` et `schemas/domain/<pack>/` en dev.

### 5.3 Variables d’environnement (extraits)

```
GA_LLAMA_PATH=~/.cache/genaptitude/models/
GA_RAG_URL=http://localhost:6333         # Qdrant
GA_RULES_URL=http://localhost:8080       # Drools/OpenFisca proxy
GA_LEDGER_URL=grpc://ledger.local:7051   # Fabric peer (optionnel)
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
```

---

## 6. Sécurité

- **ZTA local** : surface restreinte, appels sortants contrôlés.
- **CSP** : pas d’évaluations dynamiques, _allowlist_ stricte Tauri.
- **Sandbox** : exécutions non fiables en **WASI** (aucun accès FS/réseau sans cap).
- **Secrets** : stockage OS (keyring) ; pas de secrets en clair dans `tauri.conf.json`.
- **Supply chain** : `cargo deny`, SBOM (CycloneDX/SPDX), signature artifacts (cosign).

---

## 7. Observabilité

- **OTel** : traces (invokes, RAG, rules), métriques (latence p95, RAM, E2E), logs structurés (JSON).
- **Exports** : OTLP → Prometheus/Loki.
- **KPI** : taux HITL, % conformité règles, hallucination-rate, coût/exec, énergie/exec.

---

## 8. CI/CD (GitLab)

- **Stages** : `lint → build (web, wasm) → test (wasm) → bundle (tauri)`
- **Images** :
  - `node:20` pour web.
  - `rust:1` pour wasm/tests.
  - `rust:1-bookworm` (+ backports) pour bundling Tauri.
- **Artefacts** : `dist/`, `target/wasm32-*/release/*.wasm`, `target/release/bundle/**`.
- **Caches** : `target/`, `.cargo/registry/`, `node_modules/`, `.pnpm-store/`.
- **Gate** : pipeline “green” requis avant release.

---

## 9. Build & Run (local)

```bash
# Front
npm i
npm run dev          # http://localhost:1420
npm run build        # → ./dist

# Desktop
cargo tauri dev
cargo tauri build    # → target/release/bundle/**

# WASM
rustup target add wasm32-unknown-unknown wasm32-wasip1
cargo build --manifest-path src-wasm/Cargo.toml --target wasm32-wasip1 --release
```

---

## 10. Qualité & Tests

- **Rust** : `cargo fmt`, `cargo clippy -D warnings`, `cargo test`.
- **TS** : `tsc --noEmit`, (option) `vitest` pour unités UI/services.
- **Règles** : tests Given/When/Then + _golden files_.
- **Non-régression** : snapshots d’artefacts + hash.

---

## 11. Roadmap technique

- **v1.1** : Agent planner configurable, règles “live reload” via WASI.
- **v1.2** : Fine-tuning LoRA/QLoRA sur suites model-based.
- **v1.3** : Packaging Windows/macOS, signatures code, auto-update (diffs).

---

## 12. Annexes

### 12.1 Ports (par défaut)

- Vite dev : **1420**
- OTLP (gRPC) : **4317**
- Qdrant : **6333**

### 12.2 Arborescence repo (rappel)

```
src/ | public/ | dist/ | src-tauri/ | src-wasm/ | schemas/ | rules/ | .gitlab-ci.yml
```

### 12.3 Risques & mitigations

- **Libs WebKitGTK 4.1** indisponibles → utiliser CI (Debian bookworm-backports).
- **Boucle rebuild** (dev) → ne pas écrire sous `src-tauri/` ; utiliser `{app_data_dir}`.
- **Dérive de prompts** → gabarits versionnés, tests de conformité + règles.
