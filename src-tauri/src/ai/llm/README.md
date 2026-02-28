# Module `ai::llm` - Infrastructure Bas Niveau LLM

Ce module constitue la couche d'infrastructure (**Low-Level Layer**) de RAISE pour la communication avec les modèles de langage. Il fournit la "tuyauterie" technique permettant aux services de fonctionner sans se soucier de la complexité réseau ou de l'inférence locale.

Il supporte désormais deux modes de fonctionnement :

1.  **Client HTTP (Agnostique)** : Pour connecter des serveurs d'inférence externes (llama.cpp, vLLM) ou Cloud (Gemini).
2.  **Moteur Natif (Embedded)** : Pour exécuter des modèles (GGUF) directement dans le processus Rust via `Candle` (sans dépendance externe).

---

## 📂 Structure du Module

Voici l'organisation physique des fichiers de ce module :

```text
src-tauri/src/ai/llm/
├── mod.rs               # Point d'entrée : expose les structures et gère l'état global (NativeLlmState).
├── client.rs            # Client HTTP : gère la connexion réseau (llama.cpp/Gemini) et le Fallback.
├── candle_engine.rs     # [NOUVEAU] Moteur Natif : Inférence locale pure via HuggingFace Candle.
├── prompts.rs           # Personas : contient les constantes des "System Prompts".
├── response_parser.rs   # Nettoyeur : extrait le JSON/Code des réponses brutes.
└── tests.rs             # Validation : tests unitaires et d'intégration.

```

---

## 📊 Architecture & Flux de Données

Le système est **hybride**. Il permet de choisir le bon outil pour la bonne tâche.

### Schéma du Flux (Pipeline)

```mermaid
graph TD
    User[Interface / Agent] --> Decision{Choix Architecture}

    %% BRANCHE 1 : CLIENT HTTP (AGENTS)
    Decision -- "Mode Réseau (Agents complexes)" --> Client[LLM Client]

    subgraph Network_Flow [Flux Client HTTP]
        direction TB
        Client --> TryLocal[Tentative Local :8081]
        TryLocal -- "Timeout / Échec" --> Fallback[Secours Cloud Gemini]
        TryLocal --> RawResp[Réponse Brute]
        Fallback --> RawResp
    end

    %% BRANCHE 2 : MOTEUR NATIF (CHAT)
    Decision -- "Mode Natif (Chat Rapide)" --> Engine[Candle Engine]

    subgraph Native_Flow [Flux Embarqué Rust]
        direction TB
        Engine --> Load[Chargement GGUF RAM]
        Load --> Infer[Inférence Metal/CUDA/CPU]
        Infer --> Tokenizer[Décodage Tokenizer]
    end

    %% CONVERGENCE ET SORTIE
    RawResp --> Parser[Response Parser]
    Tokenizer --> Output[Sortie Texte Standardisée]
    Parser --> Output

```

### Description des Moteurs

1. **Le Client HTTP (`client.rs`)** :

- Utilisé par les **Agents Autonomes** (Software, Intent, etc.).
- Avantage : Peut utiliser des modèles énormes (70B+) hébergés sur un serveur dédié ou dans le Cloud.
- Résilience : Bascule sur Gemini si le serveur local est éteint.

2. **Le Moteur Natif (`candle_engine.rs`)** :

- Utilisé par le **Chat Direct** ou les tâches rapides.
- Avantage : **Zéro configuration**. Pas besoin de Docker ni de Python. L'application télécharge et lance le modèle (ex: Llama 3.2 1B) toute seule.
- Performance : Utilise l'accélération matérielle (Metal sur Mac, CUDA sur Nvidia, AVX sur CPU).

---




---

## ⚙️ Configuration Requise

Variables d'environnement (fichier `.env`) :

### Configuration Client HTTP (Agents)

| Variable           | Description                                                                  |
| ------------------ | ---------------------------------------------------------------------------- |
| `RAISE_LOCAL_URL`  | URL du serveur d'inférence local (ex: `http://localhost:8081`)               |
| `LLM_MODEL_FILE`   | Fichier modèle chargé par le serveur (ex: qwen2.5-1.5b-instruct-q4_k_m.gguf) |
| `RAISE_GEMINI_KEY` | Clé API Google (Backup)                                                      |

### Configuration Moteur Natif (Rust/Candle)

Si ces variables ne sont pas définies, des valeurs par défaut (Llama 3.2 1B) sont utilisées.

| Variable                  | Description                      | Exemple / Recommandé                   |
| ------------------------- | -------------------------------- | -------------------------------------- |
| `LLM_RUST_REPO_ID`        | Dépôt HuggingFace du modèle GGUF | `bartowski/Llama-3.2-1B-Instruct-GGUF` |
| `LLM_RUST_MODEL_FILE`     | Nom du fichier GGUF spécifique   | `Llama-3.2-1B-Instruct-Q4_K_M.gguf`    |
| `LLM_RUST_TOKENIZER_REPO` | Dépôt contenant `tokenizer.json` | `unsloth/Llama-3.2-1B-Instruct`        |

---

## ✅ Validation

### Tester la logique générale

Pour vérifier le parser et le client HTTP :

```bash
cargo test ai::llm

```

### Tester le Moteur Natif (Téléchargement + Inférence)

**Attention :** Ce test télécharge le modèle (~700 Mo) lors de la première exécution.

```bash
cargo test candle_engine -- --ignored

```

```

```
