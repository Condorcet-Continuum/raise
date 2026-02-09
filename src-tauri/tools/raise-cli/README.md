# 🚀 RAISE CLI

**Version:** 0.1.0
**Statut:** Stable (36 tests passés)
**Architecture:** Rust (Wrapper sur `raise-core`)

Le **Raise CLI** est l'interface de pilotage "headless" du moteur **RAISE**. Il permet aux ingénieurs système, aux auditeurs et aux pipelines CI/CD d'interagir avec le cœur Neuro-Symbolique sans passer par l'interface graphique Tauri.

Il respecte strictement l'architecture **Clean Onion** et les "Golden Rules" de sécurité (Log structuré, FS abstrait).

---

## 📦 Installation & Build

Le CLI est un binaire Rust autonome situé dans l'espace de travail Tauri.

```bash
# Depuis la racine du projet
cd src-tauri/tools/raise-cli

# Build en mode release
cargo build --release

# Exécution directe
./target/release/raise-cli --help

```

---

## 🛠 Commandes & Modules

Le CLI est organisé en 4 piliers majeurs correspondant à l'architecture du moteur.

### 1. Ingénierie & Modélisation (Model-Based)

Pilotez le cycle de vie **Arcadia** et la transformation vers le code.

| Commande       | Sous-commande     | Description                                                            |
| -------------- | ----------------- | ---------------------------------------------------------------------- |
| `model-engine` | `load --path <F>` | Charge un modèle (.aird, .json) en mémoire via le `ModelLoader`.       |
| `model-engine` | `validate`        | Lance le `ConsistencyChecker` pour vérifier les règles sémantiques.    |
| `model-engine` | `transform <DOM>` | Projette le modèle vers un domaine : `software`, `hardware`, `system`. |
| `code-gen`     | `generate <ID>`   | Génère le code source pour un composant. Supporte le **Round-Trip**.   |
| `spatial`      | `topology`        | Génère la structure 3D procédurale des couches Arcadia.                |
| `spatial`      | `health`          | Audit de stabilité (Vibration) sur le Jumeau Numérique.                |

> **Note CodeGen :** La commande `generate` supporte les langages `Rust`, `Cpp`, `TypeScript`, `Verilog`, `Vhdl`. Pour Rust, elle exécute automatiquement `cargo clippy --fix`.

### 2. Intelligence & Décision (Neuro-Symbolic)

Gérez les moteurs d'optimisation et l'exécution des workflows.

| Commande   | Sous-commande    | Description                                                           |
| ---------- | ---------------- | --------------------------------------------------------------------- |
| `workflow` | `submit-mandate` | Compile une politique de gouvernance (Mandat) en Workflow exécutable. |
| `workflow` | `set-sensor`     | Injecte une valeur simulée dans le Jumeau Numérique (Digital Twin).   |
| `workflow` | `resume`         | Débloque une étape HITL (Human-In-The-Loop) en attente de validation. |
| `genetics` | `evolve`         | Lance l'optimiseur **NSGA-II** (Pop, Gen, Mutation Rate).             |
| `ai`       | `parse`          | Teste le moteur NLP sur une phrase en langage naturel.                |
| `plugins`  | `load`           | Charge dynamiquement un bloc cognitif **WASM** sécurisé.              |

### 3. Données & Traçabilité (Sovereign Data)

Manipulez la base de données JSON et auditez les changements.

| Commande       | Sous-commande  | Description                                                      |
| -------------- | -------------- | ---------------------------------------------------------------- |
| `jsondb`       | `query/insert` | Opérations CRUD directes sur la base NoSQL transactionnelle.     |
| `traceability` | `audit`        | Lance le `Tracer` sur le `ProjectModel` actuel.                  |
| `traceability` | `impact <ID>`  | Analyse de propagation des changements (Dependency Graph).       |
| `blockchain`   | `vpn-check`    | Vérifie l'état du maillage P2P (Innernet) et du Ledger (Fabric). |

### 4. Utilitaires Système

| Commande    | Sous-commande | Description                                                  |
| ----------- | ------------- | ------------------------------------------------------------ |
| `validator` | `check`       | Vérifie l'intégrité de la structure du projet sur le disque. |
| `utils`     | `ping`        | Test de connectivité simple avec le noyau.                   |

---

## ⚡ Scénarios d'Utilisation

### A. Cycle de Génération de Code (Round-Trip)

Ce scénario charge un modèle, vérifie sa validité, et génère du code Rust propre tout en préservant les modifications manuelles.

```bash
# 1. Charger et valider le modèle
raise-cli model-engine load --path ./my_project.json
raise-cli model-engine validate

# 2. Générer le code Rust pour le composant "Logical_CPU"
raise-cli code-gen generate "Logical_CPU" --lang rust

# 3. (Optionnel) Vérifier l'impact si on modifie ce composant
raise-cli traceability impact "Logical_CPU"

```

### B. Simulation d'Incident Jumeau Numérique

Simulez une vibration anormale et observez la réaction du Workflow.

```bash
# 1. Lancer un workflow de surveillance
raise-cli workflow run "monitoring-wf"

# 2. Injecter une anomalie capteur (Vibration élevée)
raise-cli workflow set-sensor --value 8.5

# 3. Le workflow se met en pause (GatePolicy). Un opérateur valide :
raise-cli workflow resume --instance-id "inst-123" --node-id "gate-safety" --approved

```

---

## 🏗 Architecture & Tests

Le CLI est une interface mince ("Thin Client"). Il ne contient pas de logique métier lourde ; il délègue tout aux crates du workspace `raise`.

- **Gestion des Erreurs :** Utilise `raise::utils::AppError` pour des codes d'erreur unifiés.
- **Logs :** Utilise les macros `user_info!`, `user_success!` pour un feedback standardisé.
- **Système de Fichiers :** Passe exclusivement par `raise::utils::fs` (abstraction sécurisée).

### Exécuter les Tests

La suite de tests valide chaque commande et ses arguments.

```bash
cargo test

# Résultat attendu :
# test result: ok. 36 passed; 0 failed; ...

```

---

## 🧩 Modules Connectés

- [x] **AI Orchestrator** (NLP/Intent)
- [x] **Genetics Engine** (NSGA-II)
- [x] **JsonDB** (ACID Transactional)
- [x] **Workflow Engine** (State Machine)
- [x] **Model Engine** (Arcadia Metamodel)
- [x] **Code Generator** (Polyglot)
- [x] **Traceability** (Impact Analysis)
- [x] **Blockchain** (Fabric/Innernet)
- [x] **Plugins** (WASM Runtime)
- [x] **Spatial** (Procedural 3D)

---

_Generated by RAISE Assistant - 2026_
