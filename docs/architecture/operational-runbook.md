# GenAptitude — Operational Runbook (MVP)
**Version :** 1.0 · **Date :** 2025-11-08 · **Auteur :** GenAptitude  
**Slogan :** *From Business Needs to Running Code*

> Guide d’exploitation **workstation-first** (Ubuntu) et **CI GitLab** pour le MVP GenAptitude. Style **checklist** et commandes prêtes à copier/coller. Distinction **Software / System / Hardware**.

---

## 1) Objectif
- Développer, tester, **packager** et publier GenAptitude **sans dépendance cloud** côté exécution.  
- Assurer **traçabilité**, **reproductibilité** et **débogage rapide**.

---

## 2) Pré-requis Système (Ubuntu poste local)
```bash
# Outils de base
sudo apt-get update
sudo apt-get install -y curl ca-certificates git build-essential pkg-config

# Dépendances Tauri (UI GTK/WebKit); versions exactes varient selon la distro
sudo apt-get install -y libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev   libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev   patchelf desktop-file-utils appstream || true
# Remarque: si certaines libs 4.1 sont indisponibles sur votre Ubuntu,
# utilisez les bundles CI GitLab (AppImage/deb/rpm) plutôt que de packager localement.
```

**Rust & Node**
```bash
# Rust
curl https://sh.rustup.rs -sSf | sh -s -- -y
source ~/.cargo/env
rustup toolchain install stable
rustup target add wasm32-unknown-unknown wasm32-wasip1

# Node & Corepack (pnpm/yarn si souhaité)
sudo apt-get install -y nodejs npm
corepack enable || true
```

---

## 3) Setup du repo (local)
```bash
git clone <ssh-or-https-url> genaptitude && cd genaptitude
# Front: install
npm install
# WASM (exemple)
cargo build --manifest-path src-wasm/Cargo.toml --target wasm32-wasip1 --release
# Copie éventuelle vers public/wasm/
mkdir -p public/wasm && cp target/wasm32-wasip1/release/*.wasm public/wasm/ 2>/dev/null || true
```

---

## 4) Démarrage & Build (local)
```bash
# Dev navigateur
npm run dev    # http://localhost:1420

# Dev desktop (Tauri lance Vite via beforeDevCommand)
cargo tauri dev

# Build production
npm run build                      # → dist/
cargo tauri build                  # → target/release/bundle/**
```

**Smoke tests (local)**
- `dist/index.html` existe et s’ouvre dans un navigateur.  
- L’app Tauri démarre, affiche la fenêtre, et peut lire/écrire des schémas via `invoke`.  
- `public/wasm/ga_wasm.wasm` résolu depuis l’UI (test fetch + instantiate).

---

## 5) Exploitation CI/CD (GitLab)
**Pipeline (stages)** : `lint → build (web/wasm) → test (wasm) → bundle (tauri)`

**Déclenchement**
- `git commit -m "feat: …"` puis `git push`.  
- Sur la page du pipeline : *Retry* pour relancer un job, *Clear runner caches* pour purger les caches.

**Récupération d’artefacts**
- Job `web:build` → `dist/` (zip).  
- Job `wasm:build` → `target/wasm32-*/release/*.wasm`.  
- Job `tauri:bundle` → `target/release/bundle/**` (**AppImage/.deb/.rpm**).

**Purge cache CI si incohérences**
```bash
# Dans l’UI GitLab: Pipelines → (⋯) Clear runner caches
```

---

## 6) Release & Versioning
```bash
# 1) Bump version (tauri.conf.json / package.json si applicable)
# 2) Tag & push
git tag v0.1.0
git push origin v0.1.0
# 3) Le pipeline de main + tag publie les bundles (artefacts)
```
- Conservez un **CHANGELOG.md** minimal.  
- Joignez les **hash SHA256** des bundles en release.

---

## 7) Dépannage (Troubleshooting)

### 7.1 Problèmes courants (local)
| Symptôme | Cause probable | Correctif |
|---|---|---|
| Boucle `cargo tauri dev` rebuild | Écritures dans `src-tauri/` | Écrire dans `{{app_data_dir}}` via `AppHandle.path()` |
| Page blanche en build desktop | `dist/` manquant | `npm run build` puis `cargo tauri build` |
| 404 sur WASM | Fichier absent | Placer sous `public/wasm/…` avant build |
| `@tauri-apps/api` introuvable | Dépendance manquante | `npm i @tauri-apps/api` et importer depuis `@tauri-apps/api/core` |
| Vite “Could not resolve entry module index.html” | Mauvais root | Utiliser `src/index.html` + `vite.config.ts` adapté |

### 7.2 Problèmes courants (CI)
| Symptôme | Cause probable | Correctif |
|---|---|---|
| `tauri: command not found` | CLI pas dans PATH | Utiliser `cargo tauri ...` ou `cargo install tauri-cli` |
| `libsoup-3.0` / `javascriptcoregtk-4.1` introuvables | Paquets manquants | Installer `libjavascriptcoregtk-4.1-dev` `libsoup-3.0-dev` (CI) |
| `pkg-config exited with status code 1` | pkg-config absent | `apt-get install -y pkg-config` (CI) |
| `npm: not found` | Node manquant dans job | Ajouter `node:20` ou installer Node dans le job concerné |
| Artefacts non uploadés | Mauvais chemin | Vérifier chemin relatif depuis `/builds/<group>/<project>` |

---

## 8) Santé & Diagnostic
```bash
# Versions
node -v && npm -v
rustc -V && cargo -V
cargo tauri --version

# Tauri deps (Ubuntu)
dpkg -l | egrep -i 'webkit2gtk|javascriptcoregtk|libsoup3|gtk-3'

# Ports (Vite par défaut)
ss -lntp | grep 1420 || true

# Nettoyages rapides
git clean -xfd -e node_modules -e target
rm -rf dist .turbo .vite .parcel-cache 2>/dev/null || true
```

---

## 9) Sécurité & Secrets
- Utiliser **SSH** pour Git (`ssh-keygen -t ed25519` + clés GitLab).  
- Pas de secrets en clair dans `tauri.conf.json` ; préférer le **keyring OS**.  
- Restreindre les API Tauri (allowlist) et **CSP** strict.

---

## 10) Monit & Observabilité (local)
Variables d’environnement (exemples) :
```bash
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
export GA_RAG_URL=http://localhost:6333
export GA_RULES_URL=http://localhost:8080
```
- Export OTel → Prometheus/Loki via un agent local (ou docker-compose).  
- KPI : latence p95, % conformité règles, taux HITL, **hallucination-rate**, coût/exec, énergie/exec.

---

## 11) Rollback & Incidents
- **Rollback code** : `git revert <sha>` ou checkout vers **tag** stable → push → pipeline.  
- **Rollback artefacts** : réinstaller le bundle **n-1**.  
- **Freeze** : pinner versions (Rust toolchain, Node, images CI) si régression de dépendances.

---

## 12) Annexes — Rappels utiles
```bash
# Build WASM (wasip1)
cargo build --manifest-path src-wasm/Cargo.toml --target wasm32-wasip1 --release

# Copier l’artifact WASM servi par le front
mkdir -p public/wasm && cp target/wasm32-wasip1/release/*.wasm public/wasm/ 2>/dev/null || true

# Build desktop
npm run build && cargo tauri build

# Lister bundles
find target/release/bundle -maxdepth 3 -type f -printf '%P\n' 2>/dev/null || true
```
