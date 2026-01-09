# RAISE Technical Architecture

This document outlines the high-level architecture of **RAISE**, a Model-Based AI Engineering (MBAIE) platform designed with a **Local-First** approach.

The system relies on a hybrid **Rust/WASM** architecture (Performance & Safety) orchestrated by a **React/TypeScript** interface (User Experience).

---

## 🗺️ The Big Picture (Logical View)

The architecture follows a "Sandwich" model: a rich interface accelerated by WebAssembly, interacting with a robust Rust system engine via the Tauri bridge.

```text
                                     USER
                                       │
┌──────────────────────────────────────▼───────────────────────────────────────┐
│  🖥️  PRESENTATION LAYER (React Frontend)                        📂 src/     │
│  ┌────────────────────────────────────────────────────────────────────────┐  │
│  │  UNIFIED WORKSPACE (IDE)                                               │  │
│  │  ┌──────────────┐  ┌───────────────┐  ┌───────────────┐  ┌──────────┐  │  │
│  │  │ 🤖 AI Chat   │  │ 📐 Diagrams   │  │ 📝 Code Edit  │  │ ⚙️ Dash  │  │  │
│  │  └──────┬───────┘  └───────┬───────┘  └───────┬───────┘  └────┬─────┘  │  │
│  └─────────┼──────────────────┼──────────────────┼───────────────┼────────┘  │
├────────────┼──────────────────┼──────────────────┼───────────────┼───────────┤
│  🚀 WASM ACCELERATOR (Shared Logic)              ▼               ▼           │
│     📂 src-wasm/                                                             │
│  ┌────────────────────┐  ┌────────────────────┐  ┌────────────────────────┐  │
│  │ ⚡ Analyzers       │  │ 🛡️ Validators      │  │ 🔄 Model Parsers       │  │
│  │ (Consistency)      │  │ (Syntax Check)     │  │ (Fast Feedback)        │  │
│  └────────────────────┘  └────────────────────┘  └────────────────────────┘  │
├──────────────────────────────────────────────────────────────────────────────┤
│  🌉 TAURI BRIDGE (IPC / Commands / Events)                                   │
├──────────────────────────────────────────────────────────────────────────────┤
│  ⚙️  SYSTEM ENGINE (Rust Backend)                            📂 src-tauri/  │
│                                                                              │
│  ┌── [ NEURO ] ───────────┐      ┌── [ ORCHESTRATION ] ──┐      ┌── [ SYMBOLIC ] ──┐
│  │ 🧠 AI & AGENTS         │      │ ⚡ WORKFLOW ENGINE    │      │ 📐 MODEL ENGINE    │
│  │ 📂 ai/agents/          │◄────►│ 📂 workflow_engine/   │◄────►│ 📂 model_engine/   │
│  │ - Business / Soft / Hard│     │ - Scheduler           │      │ - Arcadia / Capella│
│  │ - LLM Context / RAG    │      │ - State Machine       │      │ - Transformers     │
│  └────────────────────────┘      └───────────┬───────────┘      └────────────────────┘
│                                              │
│               ┌──────────────────────────────▼──────────────────────────────┐
│               │ 💾 INFRASTRUCTURE & SOVEREIGNTY (Local-First)               │
│               │ ┌──────────────────┐  ┌──────────────────┐  ┌─────────────┐ │
│               │ │ 🗄️ JSON_DB       │  │ 🔍 TRACEABILITY  │  │ ⛓️ BLOCKCHAIN│ │
│               │ │ (ACID/BTree/WAL) │  │ (DO-178C/Audit)  │  │ (Fabric/VPN)│ │
│               │ └──────────────────┘  └──────────────────┘  └─────────────┘ │
│               └─────────────────────────────────────────────────────────────┘
└──────────────────────────────────────────────────────────────────────────────┘

```

---

## 🏗️ Layer Descriptions

### 1. Presentation Layer (Frontend)

**Location:** `src/`
This layer is not just a web page; it is a full-featured IDE. It manages the visual state and user interaction.

- **`components/diagram-editor`**: Graphical rendering engine for Arcadia/Capella models.
- **`components/ai-chat`**: Advanced conversational interface capable of displaying AI-generated artifacts (tables, code, charts).
- **`components/model-viewer`**: Explorer for models and technical data.

### 2. Acceleration Layer (WebAssembly)

**Location:** `src-wasm/`
Critical Rust modules compiled to `.wasm` to run directly in the browser.

- **Goal:** Provide instant feedback (<10ms) to the user without waiting for the backend.
- **Usage:** Real-time syntax validation, diagram consistency checking (`analyzer-consistency`), fast parsing.

### 3. System Core (Rust Backend)

**Location:** `src-tauri/src/`
The application's brain, divided into three main pillars:

#### A. Neuro Pillar (Creative Intelligence)

- **`ai/agents/`**: Multi-agent system specialized by domain (Business, Software, Hardware, EPBS).
  llama.cpp- **`ai/llm/`**: Prompt management and abstraction of model providers (llama.cpp, etc.).
- **`ai/context/`**: Conversational memory management (RAG).

#### B. Symbolic Pillar (Engineering Rigor)

- **`model_engine/`**: Implementation of the Arcadia metamodel and Capella compatibility.
- **`model_engine/transformers/`**: Converts fuzzy intentions (natural language) into structured models.
- **`rules_engine/`**: Formal validation engine (AST) ensuring models respect physical and logical constraints.

#### C. Infrastructure Pillar (Persistence)

- **`json_db/`**: Proprietary database engine written in Rust.
- Supports Transactions (WAL), BTree Indexes, and SQL.
- Ensures data remains local (secured JSON files).

- **`traceability/`**: Ensures compliance with critical standards (DO-178C, ISO-26262).
- **`blockchain/`**: Connectors for action notarization (Hyperledger Fabric).

---

## 🔄 Data Flow: The Neuro-Symbolic Loop

The key concept of RAISE is to never blindly trust the AI. Here is the lifecycle of a request:

1. **Intention**: The user expresses a need ("Add a battery to the system").
2. **Classification**: The `ai/agents/intent_classifier` determines which Agent acts (e.g., Hardware Agent).
3. **Proposition**: The Agent generates a potential model modification.
4. **Transformation**: `dialogue_to_model` converts this proposition into a strict data structure.
5. **Validation**: The `rules_engine` checks technical validity (e.g., "Is voltage compatible?").

- 🛑 _If invalid_: The AI receives the error and must correct its proposition.
- ✅ _If valid_: The modification is accepted.

6. **Commit**:

- Data is written to `json_db` (ACID transaction).
- An audit trace is generated in `traceability`.

---

## 🛠️ Tech Stack

| Domain       | Technologies                                       |
| ------------ | -------------------------------------------------- |
| **Frontend** | React, TypeScript, Vite, TailwindCSS               |
| **Backend**  | Rust, Tauri, Tokio (Async)                         |
| **WASM**     | `wasm-bindgen`, Rust                               |
| **Database** | Custom Engine (Rust), Serde, SQL Parser            |
| **AI/ML**    | LLM (Local via llama.cpp/Rust-Bert), Vector Stores |
| **Modeling** | JSON-LD, Arcadia Metamodel                         |
| **Security** | VPN (Innernet), Ed25519 (Signatures)               |

````

### ✅ Next Step

Just like for the French version, update your main English documentation index at **`docs/en/README.md`**:

```markdown
## 📚 Technical Documentation

To understand how the system works under the hood, check out:
👉 [Technical Architecture & Big Picture](./ARCHITECTURE.md)

````
