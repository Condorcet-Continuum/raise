# Module de Commandes (Tauri API Layer)

Ce répertoire contient l'ensemble des **Commandes Tauri** qui servent d'interface (API) entre le frontend (React/TypeScript) et le moteur backend (Rust).

Chaque fichier ici expose des fonctions annotées avec `#[tauri::command]`, qui sont enregistrées dans le `main.rs` et appelables depuis l'UI via `invoke()`.

## 📂 Organisation des Modules

| Fichier                        | Domaine                | Description                                                                                                     |
| :----------------------------- | :--------------------- | :-------------------------------------------------------------------------------------------------------------- |
| **`ai_commands.rs`**           | 🧠 IA Générative       | Gestion du chat avec les LLM (Local/Cloud), classification d'intention et RAG (Retriaval Augmented Generation). |
| **`blockchain_commands.rs`**   | 🔗 Blockchain & Réseau | Interactions avec Hyperledger Fabric (transactions) et le VPN Innernet (Mesh networking).                       |
| **`codegen_commands.rs`**      | ⚡ Génération de Code  | Transformation des modèles en code source (Rust, Python) via des templates.                                     |
| **`cognitive_commands.rs`**    | 🤖 Analyse Cognitive   | Exécution de modules WASM (WebAssembly) pour l'analyse structurelle ou sémantique.                              |
| **`genetics_commands.rs`**     | 🧬 Optimisation        | Algorithmes génétiques pour l'optimisation des architectures (simulation de générations).                       |
| **`json_db_commands.rs`**      | 💾 Base de Données     | CRUD complet sur le moteur NoSQL local (Spaces, DBs, Collections, Documents, Index).                            |
| **`model_commands.rs`**        | 🏗️ Gestion du Modèle   | Chargement et maintien en mémoire du `ProjectModel` (Arcadia) pour les opérations lourdes.                      |
| **`traceability_commands.rs`** | 🔍 Traçabilité & Audit | Moteur d'analyse d'impact, matrices de couverture et vérification de conformité (EU AI Act, DO-178C).           |

---

## 🛠 Détails des APIs

### 1. Intelligence Artificielle (`ai_commands.rs`)

Gère l'assistant contextuel.

- `ai_chat(user_input)`: Pipeline complet (Classification -> Recherche Contexte -> Prompting -> LLM). Supporte le mode Dual (Gemini/Local).

### 2. Blockchain & VPN (`blockchain_commands.rs`)

Interface pour la sécurité et la traçabilité distribuée.

- `fabric_submit_transaction(...)`: Soumission de transactions au ledger.
- `vpn_network_status()`: État de la connexion mesh (pairs connectés, IP).

### 3. Base de Données (`json_db_commands.rs`)

Interface directe avec le moteur de stockage JSON.

- **Structure** : Space ➝ DB ➝ Collection ➝ Document.
- **Commandes** : `create_db`, `insert_document`, `execute_query` (recherche complexe), `list_all`.

### 4. Modèle & Architecture (`model_commands.rs`)

- `load_project_model(space, db)`: Charge l'intégralité du projet depuis la DB vers la RAM (Mutex global) pour permettre les analyses rapides. S'exécute dans un thread bloquant pour ne pas figer l'UI.

### 5. Traçabilité & Conformité (`traceability_commands.rs`)

Nouvelles commandes pour l'assurance qualité.

- `analyze_impact(element_id, depth)`: Calcule la propagation d'un changement dans le graphe.
- `run_compliance_audit()`: Lance les checkers (DO-178C, ISO-26262, EU AI Act) et retourne un rapport JSON.
- `get_traceability_matrix()`: Génère la matrice de couverture SA ➔ LA.
- `get_element_neighbors(element_id)`: Retourne les parents/enfants pour la navigation visuelle.

### 6. Modules Avancés

- **Génétique** (`genetics_commands.rs`): `run_genetic_optimization` prend des paramètres de mutation/génération et simule une convergence.
- **Cognitif** (`cognitive_commands.rs`): `run_consistency_analysis` charge dynamiquement un binaire `.wasm` selon l'environnement (Dev/Prod) pour analyser le modèle.
- **CodeGen** (`codegen_commands.rs`): `generate_source_code` produit du code textuel basé sur les métadonnées du modèle.

---

## 💻 Exemple d'appel (Frontend)

Voici comment appeler ces commandes depuis React/TypeScript :

```typescript
import { invoke } from '@tauri-apps/api/core';

// Exemple : Lancer un audit de conformité
async function runAudit() {
  try {
    const report = await invoke('run_compliance_audit');
    console.log('Rapport de conformité :', report);
  } catch (error) {
    console.error("Erreur d'audit :", error);
  }
}

// Exemple : Chat AI
async function sendMessage(text: string) {
  const response = await invoke('ai_chat', { userInput: text });
  console.log('Réponse AI :', response);
}
```

````

## ⚠️ Notes Techniques

- **État Partagé (`AppState`)** : Les commandes `model_commands` et `traceability_commands` partagent le même `Mutex<ProjectModel>`. Il est impératif d'appeler `load_project_model` avant de lancer des analyses de traçabilité.
- **Async/Sync** : Les opérations lourdes (IA, Chargement Modèle, Génétique) sont `async` pour ne pas bloquer le thread principal de Tauri.

<!-- end list -->

```


````
