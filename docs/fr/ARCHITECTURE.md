# Architecture Technique RAISE

Ce document décrit l'architecture de haut niveau de **RAISE**, une plateforme d'Ingénierie IA Neuro-Symbolique (MBAIE) conçue selon l'approche **Local-First**.

Le système repose sur une architecture hybride **Rust/WASM** (Performance & Sécurité) orchestrée par une interface **React/TypeScript** (Expérience Utilisateur).

---

## 🗺️ La Big Picture (Vue Logique)

L'architecture suit un modèle en "Sandwich" : une interface riche accélérée par WebAssembly, interagissant avec un moteur système Rust via le pont Tauri.

```text
                                  UTILISATEUR
                                       │
┌──────────────────────────────────────▼───────────────────────────────────────┐
│  🖥️  COUCHE DE PRÉSENTATION (Frontend React)                    📂 src/     │
│  ┌────────────────────────────────────────────────────────────────────────┐  │
│  │  ESPACE DE TRAVAIL UNIFIÉ (IDE)                                        │  │
│  │  ┌──────────────┐  ┌───────────────┐  ┌───────────────┐  ┌──────────┐  │  │
│  │  │ 🤖 AI Chat   │  │ 📐 Diagrammes │  │ 📝 Code Edit  │  │ ⚙️ Dash  │  │  │
│  │  └──────┬───────┘  └───────┬───────┘  └───────┬───────┘  └────┬─────┘  │  │
│  └─────────┼──────────────────┼──────────────────┼───────────────┼────────┘  │
├────────────┼──────────────────┼──────────────────┼───────────────┼───────────┤
│  🚀 ACCÉLÉRATEUR WASM (Shared Logic)             ▼               ▼           │
│     📂 src-wasm/                                                             │
│  ┌────────────────────┐  ┌────────────────────┐  ┌────────────────────────┐  │
│  │ ⚡ Analyseurs      │  │ 🛡️ Validateurs     │  │ 🔄 Parsers Modèles     │  │
│  │ (Consistency)      │  │ (Syntax Check)     │  │ (Fast Feedback)        │  │
│  └────────────────────┘  └────────────────────┘  └────────────────────────┘  │
├──────────────────────────────────────────────────────────────────────────────┤
│  🌉 TAURI BRIDGE (IPC / Commands / Events)                                   │
├──────────────────────────────────────────────────────────────────────────────┤
│  ⚙️  MOTEUR SYSTÈME (Backend Rust)                           📂 src-tauri/  │
│                                                                              │
│  ┌── [ NEURO ] ───────────┐      ┌── [ ORCHESTRATION ] ──┐      ┌── [ SYMBOLIQUE ] ──┐
│  │ 🧠 IA & AGENTS         │      │ ⚡ WORKFLOW ENGINE    │      │ 📐 MODEL ENGINE    │
│  │ 📂 ai/agents/          │◄────►│ 📂 workflow_engine/   │◄────►│ 📂 model_engine/   │
│  │ - Business / Soft / Hard│     │ - Scheduler           │      │ - Arcadia / Capella│
│  │ - LLM Context / RAG    │      │ - State Machine       │      │ - Transformers     │
│  └────────────────────────┘      └───────────┬───────────┘      └────────────────────┘
│                                              │
│               ┌──────────────────────────────▼──────────────────────────────┐
│               │ 💾 INFRASTRUCTURE & SOUVERAINETÉ (Local-First)              │
│               │ ┌──────────────────┐  ┌──────────────────┐  ┌─────────────┐ │
│               │ │ 🗄️ JSON_DB       │  │ 🔍 TRACEABILITY  │  │ ⛓️ BLOCKCHAIN│ │
│               │ │ (ACID/BTree/WAL) │  │ (DO-178C/Audit)  │  │ (Fabric/VPN)│ │
│               │ └──────────────────┘  └──────────────────┘  └─────────────┘ │
│               └─────────────────────────────────────────────────────────────┘
└──────────────────────────────────────────────────────────────────────────────┘

```

---

## 🏗️ Description des Couches

### 1. Couche de Présentation (Frontend)

**Localisation :** `src/`
Cette couche n'est pas une simple page web, c'est un IDE complet. Elle gère l'état visuel et l'interaction utilisateur.

- **`components/diagram-editor`** : Moteur de rendu graphique pour les modèles Arcadia/Capella.
- **`components/ai-chat`** : Interface conversationnelle avancée capable d'afficher des artefacts (tableaux, code, graphiques) générés par l'IA.
- **`components/model-viewer`** : Explorateur de modèles et de données techniques.

### 2. Couche d'Accélération (WebAssembly)

**Localisation :** `src-wasm/`
Modules Rust critiques compilés en `.wasm` pour s'exécuter dans le navigateur.

- **Objectif :** Fournir un feedback instantané (<10ms) à l'utilisateur sans attendre le backend.
- **Usage :** Validation de syntaxe en temps réel, vérification de cohérence des diagrammes (`analyzer-consistency`), parsing rapide.

### 3. Cœur du Système (Backend Rust)

**Localisation :** `src-tauri/src/`
Le cerveau de l'application. Il est divisé en trois piliers :

#### A. Pilier Neuro (L'Intelligence Créative)

- **`ai/agents/`** : Système multi-agents spécialisés (Business, Software, Hardware, EPBS).
- **`ai/llm/`** : Gestion des prompts et abstraction des fournisseurs de modèles (Ollama, etc.).
- **`ai/context/`** : Gestion de la mémoire conversationnelle (RAG).

#### B. Pilier Symbolique (La Rigueur Ingénierie)

- **`model_engine/`** : Implémentation du métamodèle Arcadia et compatibilité Capella.
- **`model_engine/transformers/`** : Convertit les intentions floues (texte) en modèles structurés.
- **`rules_engine/`** : Moteur de validation formelle (AST) pour garantir que les modèles respectent les contraintes physiques et logiques.

#### C. Pilier Infrastructure (La Persistance)

- **`json_db/`** : Moteur de base de données propriétaire écrit en Rust.
- Supporte les Transactions (WAL), les Index BTree et le SQL.
- Garantit que les données restent locales (fichiers JSON sécurisés).

- **`traceability/`** : Assure la conformité aux normes critiques (DO-178C, ISO-26262).
- **`blockchain/`** : Connecteurs pour la notarisation des actions (Hyperledger Fabric).

---

## 🔄 Flux de Données : La Boucle Neuro-Symbolique

Le concept clé de RAISE est de ne jamais faire confiance aveuglément à l'IA. Voici le cycle de vie d'une requête :

1. **Intention** : L'utilisateur exprime un besoin ("Ajoute une batterie au système").
2. **Classification** : L'`ai/agents/intent_classifier` détermine quel Agent doit agir (ex: Hardware Agent).
3. **Proposition** : L'Agent génère une modification potentielle du modèle.
4. **Transformation** : `dialogue_to_model` convertit cette proposition en structure de données stricte.
5. **Validation** : Le `rules_engine` vérifie la validité technique (ex: "Voltage compatible ?").

- 🛑 _Si invalide_ : L'IA reçoit l'erreur et doit corriger sa proposition.
- ✅ _Si valide_ : La modification est acceptée.

6. **Engagement** :

- Les données sont écrites dans `json_db` (ACID transaction).
- Une trace d'audit est générée dans `traceability`.

---

## 🛠️ Stack Technique

| Domaine          | Technologies                                    |
| ---------------- | ----------------------------------------------- |
| **Frontend**     | React, TypeScript, Vite, TailwindCSS            |
| **Backend**      | Rust, Tauri, Tokio (Async)                      |
| **WASM**         | `wasm-bindgen`, Rust                            |
| **Database**     | Custom Engine (Rust), Serde, SQL Parser         |
| **AI/ML**        | LLM (Local via Ollama/Rust-Bert), Vector Stores |
| **Modélisation** | JSON-LD, Arcadia Metamodel                      |
| **Sécurité**     | VPN (Innernet), Ed25519 (Signatures)            |

---

_Document généré automatiquement le 27/12/2025 pour le projet RAISE._
