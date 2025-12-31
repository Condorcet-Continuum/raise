# GenAptitude — Operational Runbook (MVP)

**Version :** 1.1 · **Date :** 2025-11-29 · **Auteur :** GenAptitude  
**Slogan :** _From Business Needs to Running Code_

> Guide d’exploitation **workstation-first** (Ubuntu) et **CI GitLab** pour le MVP GenAptitude. Style **checklist** et commandes prêtes à copier/coller. Distinction **Software / System / Hardware**.

---

## 1) Objectif

- Développer, tester, **packager** et publier GenAptitude **sans dépendance cloud** côté exécution.
- Assurer **traçabilité**, **reproductibilité** et **débogage rapide** (DB, WASM, Tauri).

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
# Rust (1.88+ recommandé)
curl [https://sh.rustup.rs](https://sh.rustup.rs) -sSf | sh -s -- -y
source ~/.cargo/env
rustup toolchain install stable
rustup target add wasm32-unknown-unknown wasm32-wasip1

# Node & Corepack (pnpm/yarn si souhaité)
sudo apt-get install -y nodejs npm
corepack enable || true
```

---

## 3\) Setup du repo (local)

```bash
git clone <ssh-or-https-url> genaptitude && cd genaptitude

# 1. Configuration de l'environnement DB (Requis pour le Backend)
export PATH_RAISE_DOMAIN="$HOME/genaptitude_domain"
mkdir -p "$PATH_RAISE_DOMAIN"

# 2. Installation Front
npm install

# 3. Compilation WASM (exemple)
cargo build --manifest-path src-wasm/Cargo.toml --target wasm32-wasip1 --release
# Copie éventuelle vers public/wasm/
mkdir -p public/wasm && cp target/wasm32-wasip1/release/*.wasm public/wasm/ 2>/dev/null || true
```

---

## 4\) Démarrage & Build (local)

```bash
# Dev navigateur
npm run dev    # http://localhost:1420

# Dev desktop (Tauri lance Vite via beforeDevCommand)
# Assurez-vous que PATH_RAISE_DOMAIN est set
cargo tauri dev

# Build production
npm run build                      # → dist/
cargo tauri build                  # → target/release/bundle/**
```

**Smoke tests (local)**

- `dist/index.html` existe et s’ouvre dans un navigateur.
- L’app Tauri démarre, affiche la fenêtre.
- **DB Check** : L'app peut écrire dans `$PATH_RAISE_DOMAIN/un2/_system`.
- **Model Engine** : La commande `load_project_model` ne retourne pas d'erreur (vérifier logs console).
- `public/wasm/ga_wasm.wasm` résolu depuis l’UI (test fetch + instantiate).

---

## 5\) Exploitation CI/CD (GitLab)

**Pipeline (stages)** : `lint → build (web/wasm) → test (wasm) → bundle (tauri)`

**Déclenchement**

- `git commit -m "feat: …"` puis `git push`.
- Sur la page du pipeline : _Retry_ pour relancer un job, _Clear runner caches_ pour purger les caches.

**Récupération d’artefacts**

- Job `web:build` → `dist/` (zip).
- Job `wasm:build` → `target/wasm32-*/release/*.wasm`.
- Job `tauri:bundle` → `target/release/bundle/**` (**AppImage/.deb/.rpm**).

**Purge cache CI si incohérences**

```bash
# Dans l’UI GitLab: Pipelines → (⋯) Clear runner caches
```

---

## 6\) Release & Versioning

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

## 7\) Dépannage (Troubleshooting)

### 7.1 Problèmes courants (local)

| Symptôme                         | Cause probable              | Correctif                                                      |
| -------------------------------- | --------------------------- | -------------------------------------------------------------- |
| Boucle `cargo tauri dev` rebuild | Écritures dans `src-tauri/` | Écrire dans `{{app_data_dir}}` ou `$PATH_RAISE_DOMAIN`         |
| Page blanche en build desktop    | `dist/` manquant            | `npm run build` puis `cargo tauri build`                       |
| 404 sur WASM                     | Fichier absent              | Placer sous `public/wasm/…` avant build                        |
| Erreur DB "Path not found"       | Variable d'env manquante    | Exporter `PATH_RAISE_DOMAIN` avant de lancer l'app             |
| Erreur "Schema not found"        | Registre DB corrompu/vide   | Vérifier `schemas/v1` dans le domaine ou réinsérer les schémas |

### 7.2 Problèmes courants (CI)

| Symptôme                                             | Cause probable    | Correctif                                                       |
| ---------------------------------------------------- | ----------------- | --------------------------------------------------------------- |
| `tauri: command not found`                           | CLI pas dans PATH | Utiliser `cargo tauri ...` ou `cargo install tauri-cli`         |
| `libsoup-3.0` / `javascriptcoregtk-4.1` introuvables | Paquets manquants | Installer `libjavascriptcoregtk-4.1-dev` `libsoup-3.0-dev` (CI) |
| `pkg-config exited with status code 1`               | pkg-config absent | `apt-get install -y pkg-config` (CI)                            |
| Artefacts non uploadés                               | Mauvais chemin    | Vérifier chemin relatif depuis `/builds/<group>/<project>`      |

---

## 8\) Santé & Diagnostic

```bash
# Versions
node -v && npm -v
rustc -V && cargo -V
cargo tauri --version

# Tauri deps (Ubuntu)
dpkg -l | egrep -i 'webkit2gtk|javascriptcoregtk|libsoup3|gtk-3'

# Vérification DB Locale
ls -R $PATH_RAISE_DOMAIN

# Nettoyages rapides
git clean -xfd -e node_modules -e target
rm -rf dist .turbo .vite .parcel-cache 2>/dev/null || true
```

---

## 9\) Sécurité & Secrets

- Utiliser **SSH** pour Git (`ssh-keygen -t ed25519` + clés GitLab).
- Pas de secrets en clair dans `tauri.conf.json` ; préférer le **keyring OS**.
- Restreindre les API Tauri (allowlist) et **CSP** strict.

---

## 10\) Monit & Observabilité (local)

Variables d’environnement (exemples) :

```bash
# Persistance
export PATH_RAISE_DOMAIN="$HOME/genaptitude_domain"
export PATH_RAISE_DATASET="$HOME/genaptitude_dataset"

# Services externes (Optionnels pour MVP local)
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
export GA_RAG_URL=http://localhost:6333
export GA_RULES_URL=http://localhost:8080

# Logs
export RUST_LOG="info,genaptitude::json_db=debug"
```

- Export OTel → Prometheus/Loki via un agent local.
- KPI : latence p95, % conformité règles, taux HITL, **hallucination-rate**.

---

## 11\) Administration Base de Données (CLI)

Utilisez `jsondb_cli` pour la maintenance sans lancer l'UI.

```bash
cd src-tauri/tools/jsondb_cli

# 1. Lister les collections
cargo run -- list-collections --space un2 --db _system

# 2. Réparer/Vérifier une collection (Query simple)
cargo run -- query --collection actors --limit 5

# 3. Exécuter une transaction de maintenance
cargo run -- transaction --file ./maintenance_ops.json
```

---

## 12\) Rollback & Incidents

- **Rollback code** : `git revert <sha>` ou checkout vers **tag** stable → push → pipeline.
- **Rollback artefacts** : réinstaller le bundle **n-1**.
- **Recovery DB** : En cas de corruption, supprimer le fichier `_wal.jsonl` (perte des dernières transactions non-committées) ou restaurer le dossier `$PATH_RAISE_DOMAIN` depuis une sauvegarde.

---

## 13\) Annexes — Rappels utiles

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

```

```
