# RAISE · Workstation-First AI Use-Case Factory

<p align="center">
  <img src="../../src/assets/images/logo-white.svg" alt="RAISE Logo" width="200">
</p>

> 🌍 **Navigation:** [Global Home](../../README.md) | **🇺🇸 English** | [🇫🇷 Français](../fr/README.md)

**RAISE** is a sovereign **Use-Case Factory** for complex engineering.

More than just a tool, it is a unified platform designed to build, deploy, and execute specialized AI assistants for **System, Software, Hardware, and AI Engineering**. Its mission is to serve as a **Digital Commons Infrastructure**, restoring engineers' control over their tools and expertise.

Unlike proprietary silos, RAISE breaks down disciplinary barriers by combining the creativity of Generative AI with the rigor of formal methods, all within a **Local-First, Transparent, and Secure** environment.

---

## 🌐 Engineering Spectrum (Multidisciplinary)

RAISE orchestrates collaboration across four critical domains through its modular architecture:

### 1. System Engineering (MBSE)

_The methodological core._

- Driven by the **Arcadia** method (OA, SA, LA, PA) via the `model_engine`.
- Architectural consistency guaranteed via **JSON-LD** semantics.

### 2. Software Engineering

_From design to code._

- Polyglot code generation (Rust, C++, Python) via the `code_generator` module.
- Quality validation and compliance with design patterns.

### 3. Hardware Engineering

_Physical constraints and integration._

- Handling of Hardware constraints (Resources, I/O) via dedicated agents.
- Modeling of physical interfaces defined in `domain-models`.

### 4. AI Engineering (Neuro-Symbolic)

_The system's intelligence._

- Architecture optimization via the `genetics` genetic engine.
- Orchestration of autonomous agents and management of cognitive `plugins`.

---

## 🏛️ Philosophy & Technical Pillars

RAISE rests on four pillars that guarantee technological independence and industrial rigor:

### 1. Sovereignty (Local-First & JSON-DB)

_Your data physically belongs to you._
The architecture rejects technological lock-in. All data is managed by a custom NoSQL engine developed in Rust (`src-tauri/src/json_db`):

- **Standard Local Storage**: Data resides in readable JSON files on your disk, validated by **JSON Schema**.
- **Integrity**: ACID transaction support via a **Write-Ahead Log (WAL)** (`_wal.jsonl`) ensuring zero data corruption.
- **`x_compute` Engine**: Automatic calculation of metadata (UUIDs, timestamps) without external dependencies.

### 2. Transparency & Rigor (Neuro-Symbolic MBAIE)

_An engineer AI, not a black box._
The **MBAIE** (Model-Based AI Engineering) approach forces AI to adhere to explicit rules:

- **Logical Validation**: A **Rules Engine** (`rules_engine`) verifies the consistency of every AI proposal before validation.
- **Hybrid Optimization**: The **Genetic Engine** (`genetics`) combines generative AI (creativity) with symbolic AI (constraints) to explore solutions.

### 3. Trust (Proof & Audit)

_Critical engineering demands irrefutable proof._

- **Compliance & Reporting**: A dedicated **Traceability** module (`traceability`) generates compliance proofs for critical standards (DO-178C, ISO-26262).
- **Fabric Blockchain**: Integrated gRPC client (`blockchain/fabric`) to anchor architectural decisions on Hyperledger Fabric, creating an immutable registry.

### 4. Sustainability & Extensibility

_Durable and modular technology._

- **Cognitive Blocks**: A **Plugin** architecture (`plugins`) allows extending AI capabilities without touching the system core.
- **Sustainable Performance**: Calculation kernel compiled in **WebAssembly** (`src-wasm`) for high-performance execution on standard workstations.

---

## 🗣️ Linguistic Strategy: The Bet on Precision

RAISE adopts a strong stance on **Cognitive Sovereignty**:

- **Code & Infrastructure (English)**: To ensure technical universality and Open Source contribution, source code, APIs, and low-level comments follow the international standard (English).
- **Semantics & Business Rules (French)**: We prioritize **French** for defining formal models, requirements, and ontologies.
  - _Why?_ French offers **grammatical rigor and semantic precision** superior to contextual English. In Neuro-Symbolic AI, this precision drastically reduces ambiguities and hallucination risks when specifying critical systems. It is a choice for **high conceptual definition**.

---

## 🛠️ Installation and Getting Started

### Prerequisites

- **Node.js 20+** (Frontend management)
- **Rust 1.88+** (Backend and WASM)
- **WASM Targets**: `rustup target add wasm32-unknown-unknown wasm32-wasip1`

### Quick Commands

1.  **Compile the WASM module** (Required for UI operation):

    ```bash
    cd src-wasm && ./build.sh && cd ..
    ```

2.  **Launch the development environment**:
    ```bash
    npm install
    cargo tauri dev
    ```

---

## 🔧 Command Line Tools (CLI)

RAISE provides a suite of tools to administer the system and validate models without the graphical interface:

### 1. DB Administration (`jsondb_cli`)

```bash
# List collections
cargo run -p jsondb_cli -- list-collections --space un2 --db _system

```

### 2. AI Debugging (`ai_cli`)

```bash
# Test intent classification
cargo run -p ai_cli -- classify "Create a thermal regulation function"

```

### 3. Schema Validator (`validator_cli`)

```bash
# Validate a data file against its schema
cargo run -p validator_cli -- --data ./data/comp.json --schema arcadia/pa/phys-comp.json

```

---

## 🏗️ Project Structure

- **`src-tauri/`**: Rust Backend. The application core.
- `ai/`: Neuro-Symbolic Orchestrator.
- `blockchain/`: Proof and security clients (Fabric, Innernet).
- `code_generator/`: Code generation engines (Rust, C++, Python).
- `genetics/`: Hybrid optimization engine (Symbolic/Generative).
- `json_db/`: Sovereign database engine.
- `model_engine/`: Arcadia/Capella formal business logic.
- `plugins/`: Cognitive blocks and modular extensions.
- `rules_engine/`: Business rule validation engine.
- `tools/`: CLI Tools (`ai_cli`, `jsondb_cli`, `validator_cli`).
- `traceability/`: Compliance and reporting engine.

- **`src-wasm/`**: High-performance calculation modules compiled to WASM.
- **`src/`**: React/TypeScript Frontend.
- **`schemas/`**: Ontologies and JSON-LD definitions.
- **`domain-models/`**: Business knowledge repositories.

---

## Contact

**RAISE — Workstation-First AI Use-Case Factory**
Contact: **zair@bezghiche.com**

```

```
