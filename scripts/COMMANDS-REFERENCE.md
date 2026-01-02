# 📝 RAISE - Référence Rapide des Commandes

## 🚀 Démarrage Rapide (Quick Start)

```bash
# 1. Tout créer et lancer automatiquement
./test-raise.sh --all

# OU étape par étape :

# 2. Créer la structure
./create-raise-structure.sh
cd raise
../add-json-db-module.sh

# 3. Installer et lancer
npm install
npm run tauri:dev
```

---

## 📦 Installation & Setup

```bash
# Prérequis système (Ubuntu 24.04)
sudo apt update && sudo apt install -y \
  libwebkit2gtk-4.0-dev build-essential curl wget file \
  libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev

# Installer Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Installer Node.js 18+ (si nécessaire)
curl -fsSL https://deb.nodesource.com/setup_18.x | sudo -E bash -
sudo apt install -y nodejs

# Installer wasm-pack
cargo install wasm-pack
```

---

## 🏗️ Commandes de Construction

```bash
# Frontend uniquement
npm run dev              # Dev server sur localhost:1420
npm run build           # Build production dans dist/

# Application Tauri (Desktop)
npm run tauri:dev       # Dev mode (hot reload)
npm run tauri:build     # Build production (exécutable)

# WASM
cd src-wasm && ./build.sh    # Compiler les modules WASM

# Rust backend seul
cd src-tauri
cargo check             # Vérifier sans compiler
cargo build             # Build debug
cargo build --release   # Build optimisé
```

---

## 🧪 Tests

```bash
# Frontend
npm run test            # Lancer tous les tests
npm run test -- --watch # Mode watch
npm run test -- --ui    # Interface UI

# Backend Rust
cd src-tauri
cargo test              # Tous les tests
cargo test --lib        # Tests de bibliothèque seulement
cargo test module_name  # Tests d'un module spécifique

# WASM
cd src-wasm
wasm-pack test --node   # Tests WASM
```

---

## 🔍 Développement & Debug

```bash
# Logs détaillés
RUST_LOG=debug npm run tauri:dev
RUST_LOG=trace npm run tauri:dev

# Watch mode Rust (recompilation auto)
cd src-tauri
cargo watch -x check
cargo watch -x "run"

# Formater le code
npm run lint            # Linter TypeScript
cd src-tauri && cargo fmt   # Formater Rust
cd src-tauri && cargo clippy # Linter Rust

# Analyser les dépendances
npm audit               # Audit npm
cd src-tauri && cargo tree  # Arbre dépendances Rust
```

---

## 🗄️ JSON Database

```bash
# Vérifier la structure du module
ls -R src-tauri/src/json_db/
ls -R src/services/json-db/

# Voir les schémas
cat domain-models/software/json-schemas/component.schema.json
cat domain-models/system/json-schemas/requirement.schema.json
cat domain-models/hardware/json-schemas/component.schema.json

# Voir les contextes JSON-LD
cat domain-models/software/jsonld-contexts/component.context.json
cat domain-models/system/jsonld-contexts/requirement.context.json
cat domain-models/hardware/jsonld-contexts/component.context.json
```

---

## 📂 Structure du Projet

```bash
# Voir l'arborescence complète
tree -L 3 -I 'node_modules|target|dist'

# Statistiques du projet
cloc .                  # Compter les lignes de code

# Taille des dossiers
du -sh */
du -sh src-tauri/target  # Cache Rust
```

---

## 🔧 Maintenance

```bash
# Nettoyer les builds
npm run clean           # (si défini)
rm -rf dist/           # Frontend build
rm -rf node_modules/   # Dépendances npm

# Nettoyer Rust
cd src-tauri
cargo clean            # Supprime target/

# Mettre à jour les dépendances
npm update             # NPM
cd src-tauri && cargo update  # Cargo

# Vérifier les versions
node --version
npm --version
rustc --version
cargo --version
wasm-pack --version
```

---

## 📊 Build & Distribution

```bash
# Build tous les formats
npm run tauri:build

# Build pour une plateforme spécifique
npm run tauri:build -- --target x86_64-unknown-linux-gnu
npm run tauri:build -- --target x86_64-pc-windows-msvc
npm run tauri:build -- --target x86_64-apple-darwin

# Trouver les binaires
find src-tauri/target/release -name "raise*"

# Créer les installeurs
# Les bundles sont dans src-tauri/target/release/bundle/
ls -la src-tauri/target/release/bundle/appimage/  # Linux AppImage
ls -la src-tauri/target/release/bundle/deb/       # Debian package
ls -la src-tauri/target/release/bundle/rpm/       # RedHat package
```

---

## 🐛 Dépannage Rapide

```bash
# Réinitialiser complètement
rm -rf node_modules dist src-tauri/target
npm install
npm run tauri:dev

# Problème de port
lsof -i :1420          # Voir qui utilise le port
kill -9 <PID>          # Tuer le processus

# Problème Rust
cd src-tauri
cargo clean
cargo update
cargo check

# Cache npm corrompu
npm cache clean --force
rm -rf node_modules package-lock.json
npm install

# Logs détaillés
RUST_BACKTRACE=1 RUST_LOG=trace npm run tauri:dev
```

---

## 🎨 Personnalisation

```bash
# Changer le nom de l'app
# Éditer src-tauri/tauri.conf.json
vim src-tauri/tauri.conf.json

# Changer l'icône
# Placer les icônes dans src-tauri/icons/
ls src-tauri/icons/

# Configuration Tailwind
vim tailwind.config.js

# Configuration TypeScript
vim tsconfig.json
```

---

## 📝 Git & Versioning

```bash
# Initialiser Git (si pas fait)
git init
git add .
git commit -m "Initial commit: RAISE structure"

# Créer une branche de développement
git checkout -b develop

# Versionner
npm version patch       # 0.1.0 → 0.1.1
npm version minor       # 0.1.1 → 0.2.0
npm version major       # 0.2.0 → 1.0.0

# Tags
git tag v0.1.0
git push --tags
```

---

## 📚 Documentation

```bash
# Générer la documentation Rust
cd src-tauri
cargo doc --open        # Ouvre dans le navigateur

# Générer la documentation TypeScript
npm run docs            # (si configuré avec TypeDoc)

# Lire la documentation
cat docs/README.md
cat docs/json-db.md
cat TESTING-GUIDE.md
```

---

## 🎯 Scripts Utiles Personnalisés

Ajoutez ces scripts dans `package.json` :

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "tauri:dev": "tauri dev",
    "tauri:build": "tauri build",
    "test": "vitest",
    "test:ui": "vitest --ui",
    "test:coverage": "vitest --coverage",
    "lint": "eslint . --ext ts,tsx",
    "lint:fix": "eslint . --ext ts,tsx --fix",
    "format": "prettier --write \"src/**/*.{ts,tsx}\"",
    "type-check": "tsc --noEmit",
    "wasm:build": "cd src-wasm && ./build.sh",
    "clean": "rm -rf dist node_modules src-tauri/target",
    "fresh": "npm run clean && npm install && npm run tauri:dev"
  }
}
```

---

## 🔗 Liens Utiles

- **Docs Tauri** : https://tauri.app/v1/guides/
- **Docs React** : https://react.dev/
- **Docs Rust** : https://doc.rust-lang.org/book/
- **Cargo Book** : https://doc.rust-lang.org/cargo/
- **WASM Book** : https://rustwasm.github.io/docs/book/
- **JSON Schema** : https://json-schema.org/
- **JSON-LD** : https://json-ld.org/

---

## ⚡ Commandes les Plus Utilisées

```bash
# Top 5 commandes de développement quotidien
npm run tauri:dev       # Lancer l'app en dev
npm run test           # Exécuter les tests
npm run lint:fix       # Corriger le style
cargo fmt              # Formater Rust (dans src-tauri/)
git status             # Vérifier l'état Git
```

---

**💡 Tip** : Créez des alias dans votre `.bashrc` ou `.zshrc` :

```bash
alias gdev="npm run tauri:dev"
alias gtest="npm run test"
alias gbuild="npm run tauri:build"
alias gclean="npm run clean"
```
