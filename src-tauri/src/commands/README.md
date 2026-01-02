# Module `commands` — Interface Tauri (Backend API)

Ce module expose toutes les fonctions Rust accessibles depuis le Frontend (React/TypeScript) via le pont Tauri `invoke()`. Il agit comme la **Couche de Contrôle** de l'architecture Hexagonale de RAISE.

## 🧩 Vue d'ensemble des Modules

Voici la liste des modules de commandes disponibles et leurs responsabilités :

| Module              | Fichier Source             | Rôle Principal                                                                   | Statut    |
| :------------------ | :------------------------- | :------------------------------------------------------------------------------- | :-------- |
| **I.A. & Agents**   | `ai_commands.rs`           | **Dispatcher** : Analyse d'intention et pilotage des Agents (OA/SA/LA/PA/IVVQ).  | ✅ Stable |
| **Base de Données** | `json_db_commands.rs`      | **CRUD & NoCode** : Gestion des collections, schémas, requêtes et règles métier. | ✅ Stable |
| **Workflow**        | `workflow_commands.rs`     | **Orchestration** : Moteur d'exécution de processus (BPMN-like).                 | ✅ Stable |
| **Traçabilité**     | `traceability_commands.rs` | **Compliance** : Analyse d'impact, matrices de couverture et audits.             | ✅ Stable |
| **Blockchain**      | `blockchain_commands.rs`   | **Sécurité** : Transactions Hyperledger Fabric et VPN Mesh.                      | 🚧 Stub   |
| **Génération Code** | `codegen_commands.rs`      | **Transpilation** : Transformation des modèles en code source (Rust/Python).     | 🚧 Beta   |
| **Cognitif**        | `cognitive_commands.rs`    | **Plugins** : Exécution de modules d'analyse WASM externes.                      | 🚧 Beta   |
| **Génétique**       | `genetics_commands.rs`     | **Optimisation** : Algorithmes évolutionnaires pour l'architecture.              | 🚧 Simu   |
| **Modèle**          | `model_commands.rs`        | **I/O Lourd** : Chargement global et gestion de la mémoire projet.               | ✅ Stable |
| **Utilitaires**     | `utils_commands.rs`        | **Système** : Infos de build, configuration et état de santé.                    | ✅ Stable |

---

## 🏛️ Architecture & Flux de Données

Les commandes servent d'aiguilleur : elles reçoivent les requêtes UI, valident les entrées, appellent les services métier, et retournent des résultats sérialisés.

```text
┌──────────────┐
│   FRONTEND   │ (React / TypeScript)
└──────┬───────┘
       │ invoke('nom_commande', { params })
       ▼
┌─────────────────────────────────────────────────────────────┐
│                      TAURI COMMANDS                         │
│                  (src-tauri/src/commands)                   │
├──────────────┬──────────────┬───────────────┬───────────────┤
│  ai_commands │ db_commands  │ flow_commands │  ...others    │
└──────┬───────┴──────┬───────┴───────┬───────┴───────┬───────┘
       │              │               │               │
       ▼              ▼               ▼               ▼
┌──────────────┐┌─────────────┐┌──────────────┐┌──────────────┐
│  AI AGENTS   ││   JSON-DB   ││   WORKFLOW   ││  BLOCKCHAIN  │
│ (Mistral/Gem)││  (Storage)  ││   ENGINE     ││   (Fabric)   │
└──────────────┘└─────────────┘└──────────────┘└──────────────┘
```

---

## 📦 Catalogue Détaillé des Commandes

### 1\. Intelligence Artificielle (`ai_commands.rs`)

Le point d'entrée pour le système multi-agents.

| Commande  | Description                                                                                                                                                                          |
| :-------- | :----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ai_chat` | **Dispatcher principal**. Analyse l'intention utilisateur et route vers l'agent approprié (Business, System, Software, Hardware, Data, Transverse) ou le mode conversationnel (RAG). |

### 2\. Base de Données & NoCode (`json_db_commands.rs`)

Gestion bas niveau du stockage JSON et des collections.

| Commande                   | Description                                                                                            |
| :------------------------- | :----------------------------------------------------------------------------------------------------- |
| `jsondb_create_collection` | Crée une collection avec ou sans schéma JSON.                                                          |
| `jsondb_insert_document`   | Insère un document (avec validation automatique du schéma).                                            |
| `jsondb_execute_query`     | Moteur de recherche structuré (filtres, tris).                                                         |
| `jsondb_evaluate_draft`    | **Simulateur de Règles** : Teste un document contre les règles métier (`x_rules`) sans le sauvegarder. |

### 3\. Workflow Engine (`workflow_commands.rs`)

Orchestration des processus métier (BPMN-like).

| Commande             | Description                                   |
| :------------------- | :-------------------------------------------- |
| `start_workflow`     | Instancie et démarre un nouveau workflow.     |
| `resume_workflow`    | Débloque une étape (ex: approbation humaine). |
| `get_workflow_state` | Récupère l'état courant (logs, nœuds actifs). |

### 4\. Traçabilité & Compliance (`traceability_commands.rs`)

Outils d'analyse d'impact et d'audit.

| Commande                  | Description                                                   |
| :------------------------ | :------------------------------------------------------------ |
| `analyze_impact`          | Calcule la propagation des changements (Upstream/Downstream). |
| `run_compliance_audit`    | Vérifie la conformité du modèle (Règles Qualité).             |
| `get_traceability_matrix` | Génère la matrice de couverture (ex: SA vers LA).             |

### 5\. Blockchain & VPN (`blockchain_commands.rs`)

Infrastructure décentralisée pour la collaboration sécurisée.

| Commande                    | Description                                            |
| :-------------------------- | :----------------------------------------------------- |
| `fabric_submit_transaction` | Enregistre une preuve immuable sur Hyperledger Fabric. |
| `vpn_network_status`        | État du réseau Mesh (Innernet/WireGuard).              |

### 6\. Génération de Code (`codegen_commands.rs`)

Transformation des modèles en code source.

| Commande               | Description                                                                      |
| :--------------------- | :------------------------------------------------------------------------------- |
| `generate_source_code` | Génère du code (Rust/Python) à partir d'un élément du modèle (ex: Composant LA). |

### 7\. Cognition & WASM (`cognitive_commands.rs`)

Exécution de plugins d'analyse avancée (WebAssembly).

| Commande                   | Description                                                           |
| :------------------------- | :-------------------------------------------------------------------- |
| `run_consistency_analysis` | Lance un plugin WASM pour analyser la cohérence sémantique du modèle. |

### 8\. Génétique (`genetics_commands.rs`)

Optimisation architecturale par algorithmes évolutionnaires.

| Commande                   | Description                                                                            |
| :------------------------- | :------------------------------------------------------------------------------------- |
| `run_genetic_optimization` | Lance une simulation pour trouver la meilleure architecture (ex: compromis Coût/Perf). |

### 9\. Modèle (`model_commands.rs`)

Chargement global du projet.

| Commande             | Description                                                                 |
| :------------------- | :-------------------------------------------------------------------------- |
| `load_project_model` | Charge l'intégralité du modèle en mémoire (opération lourde, thread dédié). |

### 10\. Utilitaires (`utils_commands.rs`)

| Commande       | Description                                                 |
| :------------- | :---------------------------------------------------------- |
| `get_app_info` | Retourne la version, l'état de l'API et le mode (Dev/Prod). |

---

## 🛠️ Ajouter une nouvelle commande

1.  Créer la fonction dans un fichier existant ou nouveau (ex: `my_commands.rs`).
2.  Annoter avec `#[tauri::command]`.
3.  Enregistrer la commande dans `src-tauri/src/lib.rs` (fonction `generate_handler!`).

<!-- end list -->

```rust
#[tauri::command]
pub fn my_custom_command(name: String) -> String {
    format!("Hello, {}!", name)
}
```

```

```
