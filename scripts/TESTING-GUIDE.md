# 🚀 Guide de Test - GenAptitude

## 📋 Méthode 1 : Script Interactif (Recommandé)

Le moyen le plus simple pour tester GenAptitude :

```bash
# 1. Rendre le script exécutable
chmod +x test-genaptitude.sh

# 2. Lancer le menu interactif
./test-genaptitude.sh
```

Le script vous guidera à travers chaque étape avec un menu :
- ✅ Vérification des prérequis
- ✅ Création de la structure
- ✅ Installation des dépendances
- ✅ Lancement en mode dev
- ✅ Tests unitaires
- ✅ Build de production

### Options en ligne de commande

```bash
# Vérifier les prérequis
./test-genaptitude.sh --check

# Tout exécuter automatiquement
./test-genaptitude.sh --all

# Lancer en mode dev uniquement
./test-genaptitude.sh --dev

# Tester le module JSON DB
./test-genaptitude.sh --json-db
```

---

## 📋 Méthode 2 : Étape par Étape Manuelle

### Étape 1 : Vérifier les Prérequis

```bash
# Node.js 18+
node --version

# Rust 1.75+
rustc --version
cargo --version

# Dépendances système Ubuntu 24.04
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.0-dev \
  build-essential \
  curl \
  wget \
  file \
  libssl-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

### Étape 2 : Créer la Structure

```bash
# 1. Créer la structure de base
chmod +x create-genaptitude-structure.sh
./create-genaptitude-structure.sh

# 2. Ajouter le module JSON Database
cd genaptitude
chmod +x ../add-json-db-module.sh
../add-json-db-module.sh
```

### Étape 3 : Installer les Dépendances

```bash
# Dans le dossier genaptitude/

# Frontend (Node.js)
npm install

# Backend (Rust)
cd src-tauri
cargo check
cd ..

# WASM tools (optionnel)
cargo install wasm-pack
```

### Étape 4 : Lancer en Mode Développement

#### Option A : Application Desktop (Tauri)

```bash
npm run tauri:dev
```

> ✅ **Recommandé** : Lance l'application desktop complète avec Rust backend

#### Option B : Frontend Web Uniquement

```bash
npm run dev
```

> Ouvre http://localhost:1420 dans votre navigateur

### Étape 5 : Exécuter les Tests

```bash
# Tests frontend (Vitest)
npm run test

# Tests Rust
cd src-tauri
cargo test
cd ..
```

### Étape 6 : Build de Production

```bash
# Créer l'exécutable
npm run tauri:build

# Le binaire sera dans:
# src-tauri/target/release/genaptitude (Linux)
# src-tauri/target/release/genaptitude.exe (Windows)
# src-tauri/target/release/bundle/ (tous les formats)
```

---

## 🧪 Tests Spécifiques

### Tester le Module JSON Database

```bash
# Vérifier la structure
ls -la src-tauri/src/json_db/
ls -la src/services/json-db/
ls -la domain-models/software/json-schemas/

# Voir les schémas JSON
cat domain-models/software/json-schemas/component.schema.json
cat domain-models/system/json-schemas/requirement.schema.json
cat domain-models/hardware/json-schemas/component.schema.json

# Voir les contextes JSON-LD
cat domain-models/software/jsonld-contexts/component.context.json
```

### Tester la Compilation WASM

```bash
cd src-wasm

# Build WASM
chmod +x build.sh
./build.sh

# Vérifier le package généré
ls -la pkg/

cd ..
```

### Tester les Commandes Tauri

Une fois l'application lancée (`npm run tauri:dev`), vous pouvez tester :

1. **Interface IA** : Ouvrez l'application et testez le chat
2. **Viewer de Modèles** : Visualisation des diagrammes
3. **Éditeur de Code** : Génération de code
4. **JSON DB** : CRUD sur les collections

---

## 🎯 Ce que Vous Devriez Voir

### ✅ Lancement Réussi

Quand vous lancez `npm run tauri:dev`, vous devriez voir :

```
   Compiling genaptitude v0.1.0
    Finished dev [unoptimized + debuginfo] target(s) in X.XXs
        Info Watching /path/to/genaptitude for changes...
    
  VITE v5.x.x  ready in XXX ms

  ➜  Local:   http://localhost:1420/
  ➜  Network: use --host to expose
  ➜  press h to show help
```

Puis une fenêtre desktop s'ouvre avec votre application.

### ✅ Application Fonctionnelle

L'interface devrait afficher :
- 🎯 Titre "GenAptitude"
- 🔵 Module Software Engineering
- 🟢 Module System Engineering
- 🟠 Module Hardware Engineering
- 🤖 UI IA Native
- Compteur interactif fonctionnel

---

## 🐛 Résolution de Problèmes

### Erreur : "command not found: tauri"

```bash
# Réinstaller Tauri CLI
npm install -D @tauri-apps/cli
```

### Erreur : Dépendances système manquantes

```bash
# Ubuntu/Debian
sudo apt install libwebkit2gtk-4.0-dev build-essential

# Fedora
sudo dnf install webkit2gtk4.0-devel

# Arch
sudo pacman -S webkit2gtk
```

### Erreur : Port 1420 déjà utilisé

```bash
# Changer le port dans vite.config.ts
server: {
  port: 1421,  // Nouveau port
  strictPort: true,
}

# Et dans src-tauri/tauri.conf.json
"devPath": "http://localhost:1421"
```

### Erreur : "failed to bundle project"

```bash
# Build en mode debug d'abord
cd src-tauri
cargo build
cd ..

# Puis retry le bundle
npm run tauri:build
```

### Rust prend trop d'espace disque

```bash
# Nettoyer les builds
cd src-tauri
cargo clean
cd ..
```

---

## 📊 Structure Attendue

Après création complète, vous devriez avoir :

```
genaptitude/
├── src/                    # Frontend React
│   ├── components/         # Composants UI
│   ├── features/          # Features par domaine
│   └── services/          # Services (dont json-db)
├── src-tauri/             # Backend Rust
│   └── src/
│       ├── commands/      # Commandes Tauri
│       ├── ai/           # Module IA
│       ├── model_engine/ # Moteur modélisation
│       ├── code_generator/
│       ├── json_db/      # 🆕 Module JSON DB
│       └── main.rs
├── src-wasm/             # Modules WASM
├── domain-models/        # Modèles métier
│   ├── software/
│   │   ├── json-schemas/     # 🆕 Schémas JSON
│   │   └── jsonld-contexts/  # 🆕 Contextes JSON-LD
│   ├── system/
│   │   ├── json-schemas/
│   │   └── jsonld-contexts/
│   └── hardware/
│       ├── json-schemas/
│       └── jsonld-contexts/
├── tests/                # Tests
├── docs/                 # Documentation
├── package.json
└── README.md
```

---

## 🎓 Prochaines Étapes

Une fois l'application lancée et fonctionnelle :

1. **Implémenter les Agents IA** (`src-tauri/src/ai/`)
2. **Développer le Moteur de Modélisation** (`src-tauri/src/model_engine/`)
3. **Connecter le Module JSON DB** au frontend
4. **Créer les Templates Arcadia/Capella**
5. **Implémenter les Générateurs de Code**

---

## 📚 Ressources

- **Tauri** : https://tauri.app/v1/guides/
- **React** : https://react.dev/
- **Rust** : https://doc.rust-lang.org/book/
- **WASM** : https://rustwasm.github.io/docs/book/
- **JSON Schema** : https://json-schema.org/
- **JSON-LD** : https://json-ld.org/

---

## 💡 Conseils

### Développement Efficace

```bash
# Terminal 1 : Watch Rust
cd src-tauri
cargo watch -x check

# Terminal 2 : Dev frontend
npm run dev

# Terminal 3 : Tests en continu
npm run test -- --watch
```

### Hot Reload

Tauri supporte le hot reload :
- **Frontend** : Changements React rechargés instantanément
- **Rust** : Recompilation automatique (plus lent)

### Debug

```bash
# Mode debug avec logs
RUST_LOG=debug npm run tauri:dev

# Chrome DevTools
F12 dans l'application Tauri
```

---

## ✅ Checklist de Test

- [ ] Prérequis installés
- [ ] Structure créée
- [ ] Dépendances installées
- [ ] Application lance en dev
- [ ] Interface s'affiche correctement
- [ ] Compteur fonctionne
- [ ] Modules (Software/System/Hardware) visibles
- [ ] Module JSON DB présent
- [ ] Tests passent
- [ ] Build production réussit

---

**🎉 Félicitations ! Votre environnement GenAptitude est prêt !**
