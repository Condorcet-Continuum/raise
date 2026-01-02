# RAISE · Usine de Cas d'Usage IA Orientée Poste de Travail

<p align="center">
<img src="src/assets/images/logo-white.svg" alt="RAISE Logo" width="200">
</p>

**RAISE** est une **Use-Case Factory** (Usine à Cas d'Usage) souveraine pour l'ingénierie complexe.

Plus qu'un simple outil, c'est une plateforme unifiée qui permet de concevoir, déployer et exécuter des assistants IA spécialisés pour l'**Ingénierie Système, Logicielle, Matérielle et IA**. Elle a pour vocation d'être une **infrastructure de Bien Commun Numérique**, redonnant aux ingénieurs la maîtrise de leurs outils et de leur savoir-faire.

Contrairement aux silos propriétaires, RAISE décloisonne les disciplines en combinant la créativité de l'IA Générative avec la rigueur des méthodes formelles, le tout dans un environnement **Local-First, Transparent et Sécurisé**.

---

## 🌐 Spectre d'Ingénierie (Multidisciplinaire)

RAISE orchestre la collaboration entre quatre domaines critiques grâce à son architecture modulaire :

### 1. Ingénierie Système (MBSE)

_Le cœur méthodologique._

- Pilotage par la méthode **Arcadia** (OA, SA, LA, PA) via le moteur `model_engine`.
- Garantie de cohérence architecturale via sémantique **JSON-LD**.

### 2. Ingénierie Logicielle

_De la conception au code._

- Génération de code polyglotte (Rust, C++, Python) via le module `code_generator`.
- Validation de la qualité et conformité aux patterns de conception.

### 3. Ingénierie Matérielle

_Contraintes physiques et intégration._

- Prise en compte des contraintes Hardware (Ressources, I/O) via des agents dédiés.
- Modélisation des interfaces physiques définie dans les `domain-models`.

### 4. Ingénierie IA (Neuro-Symbolique)

_L'intelligence du système._

- Optimisation des architectures via le moteur génétique `genetics`.
- Orchestration d'agents autonomes et gestion des `plugins` cognitifs.

---

## 🏛️ Philosophie & Piliers Techniques

RAISE repose sur quatre piliers qui garantissent l'indépendance technologique et la rigueur industrielle :

### 1. Souveraineté (Local-First & JSON-DB)

_Vos données vous appartiennent physiquement._
L'architecture refuse le verrouillage technologique. Toutes les données sont gérées par un moteur NoSQL sur-mesure développé en Rust (`src-tauri/src/json_db`) :

- **Stockage Local Standard** : Les données résident dans des fichiers JSON lisibles sur votre disque, validés par **JSON Schema**.
- **Intégrité** : Support des transactions ACID via un **Write-Ahead Log (WAL)** (`_wal.jsonl`) qui garantit qu'aucune donnée n'est corrompue.
- **Moteur `x_compute**` : Calcul automatique des métadonnées (UUID, timestamps) sans dépendance externe.

### 2. Transparence & Rigueur (MBAIE Neuro-Symbolique)

_Une IA ingénieur, pas une boîte noire._
L'approche **MBAIE** (Model-Based AI Engineering) force l'IA à respecter des règles explicites :

- **Validation Logique** : Un **Moteur de Règles** (`rules_engine`) vérifie la cohérence de chaque proposition de l'IA avant validation.
- **Optimisation Hybride** : Le **Moteur Génétique** (`genetics`) combine l'IA générative (créativité) et l'IA symbolique (contraintes) pour explorer les solutions.

### 3. Confiance (Preuve & Audit)

_L'ingénierie critique exige des preuves irréfutables._

- **Compliance & Reporting** : Un module dédié de **Traçabilité** (`traceability`) génère les preuves de conformité pour les standards critiques (DO-178C, ISO-26262).
- **Blockchain Fabric** : Client gRPC intégré (`blockchain/fabric`) pour ancrer les décisions d'architecture sur Hyperledger Fabric, créant un registre immuable.

### 4. Pérennité & Extensibilité

_Une technologie durable et modulaire._

- **Blocs Cognitifs** : Une architecture de **Plugins** (`plugins`) permet d'étendre les capacités de l'IA sans toucher au cœur du système.
- **Performance Durable** : Noyau de calcul compilé en **WebAssembly** (`src-wasm`) pour une exécution haute performance sur poste standard.

---

## 🗣️ Stratégie Linguistique : Le Pari de la Précision

RAISE adopte une position forte sur la **Souveraineté Cognitive** :

- **Code & Infrastructure (Anglais)** : Pour garantir l'universalité technique et la contribution Open Source, le code source, les APIs et les commentaires bas-niveau respectent le standard international (Anglais).
- **Sémantique & Règles Métier (Français)** : Nous privilégions le **Français** pour la définition des modèles formels, des exigences et des ontologies.
- _Pourquoi ?_ Le français offre une **rigueur grammaticale et une précision sémantique** supérieures à l'anglais contextuel. Dans l'IA Neuro-Symbolique, cette précision réduit drastiquement les ambiguïtés et les risques d'hallucinations lors de la spécification de systèmes critiques. C'est le choix de la **haute définition conceptuelle**.

---

## 🛠️ Installation et Démarrage

### Prérequis

- **Node.js 20+** (Frontend)
- **Rust 1.88+** (Backend et WASM)
- **Cibles WASM** : `rustup target add wasm32-unknown-unknown wasm32-wasip1`

### Commandes Rapides

1. **Compiler le module WASM** (Requis pour l'UI) :

```bash
cd src-wasm && ./build.sh && cd ..

```

2. **Lancer l'environnement de développement** :

```bash
npm install
cargo tauri dev

```

---

## 🔧 Outils en Ligne de Commande (CLI)

RAISE fournit une suite d'outils pour administrer le système et valider les modèles sans interface graphique :

### 1. Administration BDD (`jsondb_cli`)

```bash
# Lister les collections
cargo run -p jsondb_cli -- list-collections --space un2 --db _system

```

### 2. Débogage IA (`ai_cli`)

```bash
# Tester la classification d'intention
cargo run -p ai_cli -- classify "Crée une fonction de régulation thermique"

```

### 3. Validateur de Schéma (`validator_cli`)

```bash
# Valider un fichier de données contre son schéma
cargo run -p validator_cli -- --data ./data/comp.json --schema arcadia/pa/phys-comp.json

```

---

## 🏗️ Structure du Projet

- **`src-tauri/`** : Backend Rust. Cœur de l'application.
- `ai/` : Orchestrateur Neuro-Symbolique.
- `blockchain/` : Clients de preuve et sécurité (Fabric, Innernet).
- `code_generator/` : Moteurs de génération de code (Rust, C++, Python).
- `genetics/` : Moteur d'optimisation hybride (Symbolique/Générative).
- `json_db/` : Moteur de base de données souverain.
- `model_engine/` : Logique métier formelle Arcadia/Capella.
- `plugins/` : Blocs cognitifs et extensions modulaires.
- `rules_engine/` : Moteur de validation des règles métier.
- `tools/` : Outils CLI (`ai_cli`, `jsondb_cli`, `validator_cli`).
- `traceability/` : Moteur de conformité et reporting.

- **`src-wasm/`** : Modules de calcul haute performance compilés en WASM.
- **`src/`** : Frontend React/TypeScript.
- **`schemas/`** : Ontologies et définitions JSON-LD.
- **`domain-models/`** : Référentiels de connaissances métier.

---

## Contact

**RAISE — Usine de Cas d'Usage IA Orientée Poste de Travail**
Contact : **zair@bezghiche.com**
