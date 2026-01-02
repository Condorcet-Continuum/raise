# 🎓 AI Training & Fine-Tuning Module

This module is responsible for the **Data Preparation** phase of the Fine-Tuning pipeline. It bridges the gap between the application's runtime data (stored in JSON-DB) and the external Python training tools.

## 📂 Architecture

The Fine-Tuning workflow in RAISE is hybrid:

1.  **Rust (This Module):** Extracts high-quality conversations and documentation from the database, formats them into a structured dataset, and exports them to disk.
2.  **Python (`/tools/trainer`):** Loads this dataset to fine-tune a base LLM (e.g., Mistral, Llama 3) using QLoRA.
3.  **Rust (Inference):** Loads the resulting adapter (`.gguf`) to improve generation quality.

## 🛠️ Components

### `dataset.rs`

Contains the core logic for dataset generation.

- **Function:** `internal_export_process`
- **Command:** `ai_export_dataset`
- **Output Format:** JSONL (JSON Lines), compatible with HuggingFace `datasets`.

## 📝 Data Format

The module exports data formatted for **Instruction Tuning** (specifically tailored for Mistral/Llama prompts).

**Pattern:**

```text
<s>[INST] {Instruction / User Prompt} [/INST] {Ideal Response} </s>

```

**Example (`dataset.jsonl`):**

```json
{"text": "<s>[INST] Crée un composant logiciel. [/INST] J'ai créé le composant 'New_Component' dans la couche Logique. </s>"}
{"text": "<s>[INST] C'est quoi un Actor ? [/INST] Un Actor représente une entité externe qui interagit avec le système. </s>"}

```

## 🚀 Usage

This command is exposed to the frontend via Tauri.

### Rust (Backend Test)

```rust
use raise::ai::training::dataset;

// Exports to a temporary file
dataset::internal_export_process("/tmp/my_dataset.jsonl");

```

### TypeScript (Frontend)

```typescript
import { invoke } from '@tauri-apps/api/core';

await invoke('ai_export_dataset', {
  outputPath: '/path/to/save/dataset.jsonl',
});
```

## 🔮 Roadmap

- [ ] **Connect to Storage:** Replace static examples with real queries to `StorageEngine` (filtering by user rating).
- [ ] **Data Sanitization:** Anonymize sensitive data before export.
- [ ] **Validation Split:** Automatically split data into `train.jsonl` and `val.jsonl`.
- [ ] **Hyperparameters:** Allow passing training config (epochs, learning rate) from Rust to the Python script config.

```


```
