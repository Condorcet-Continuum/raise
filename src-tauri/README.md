# 🦀 RAISE Core Backend

Ce répertoire contient le cœur logique de **RAISE**, implémenté en **Rust**. Il s'agit du moteur souverain chargé de l'orchestration neuro-symbolique, du calcul haute performance et de la gestion de la connaissance MBSE.

## 🏗️ Architecture du Cœur

Le backend est structuré pour garantir une isolation stricte entre les entrées/sorties asynchrones et les calculs lourds (CPU-bound) afin de maintenir une réactivité maximale de l'interface utilisateur.

### 1. Moteur de Workflow & Handlers
Le `workflow_engine` pilote l'exécution séquentielle ou parallèle des tâches d'ingénierie.
* **GeneticsHandler** : Gère l'optimisation d'architecture via des algorithmes génétiques (NSGA-II).
* **WorldModelHandler** : (Nouveau) Utilise le GNN pour prédire l'impact topologique d'une modification sur le graphe système.
* **TaskHandler** : Délègue des actions spécifiques aux agents spécialisés.

### 2. Intelligence Neuro-Symbolique (`ai/`)
RAISE combine la flexibilité des LLM avec la rigueur des modèles formels.
* **Orchestrateur** : Planifie les missions et coordonne les agents.
* **World Model (GNN)** : Implémentation native via **Candle** pour une intuition topologique du graphe Arcadia.
* **RAG (Retrieval-Augmented Generation)** : Système de mémoire vectorielle locale pour l'accès documentaire.

### 3. Moteur Génétique (`genetics/`)
Module dédié à l'exploration massive de l'espace des solutions.
* **Isolation Parallèle** : Les calculs sont isolés dans un pool de threads **Rayon** via la façade `spawn_cpu_task`, évitant ainsi de saturer la boucle d'événements asynchrone **Tokio**.
* **Multi-Objectif** : Optimisation Pareto (Poids, Coût, Performance) basée sur l'ontologie Arcadia.

### 4. Persistance & Graphe (`json_db/`)
Moteur NoSQL souverain conçu pour l'intégrité MBSE.
* **JSON-LD Native** : Support natif de la sémantique de graphe pour une interopérabilité totale.
* **Transactionnel** : Support WAL (Write-Ahead Log) pour garantir la résilience des données locales.

---

## 🚦 Modèle de Concurrence : Async vs Sync

Pour garantir une performance "Zéro Dette", le Core respecte cette règle d'or :
1. **Async (Tokio)** : Utilisé pour tout ce qui est I/O (Réseau, Lecture DB, Appels LLM).
2. **Sync (Rayon)** : Utilisé pour les calculs bruts (Évaluation génétique, Algèbre tensorielle, Inférence locale).

> **Note technique** : Le `GeneticsHandler` délègue explicitement l'évolution de la population à Rayon pour saturer le CPU sans geler l'application.

---

## 🛠️ Développement & Tests

### Compilation
Le backend peut être compilé avec des accélérations matérielles spécifiques pour l'IA :
```bash
# Avec support CUDA (NVIDIA)
cargo build --features cuda

# Avec support Metal (Apple Silicon)
cargo build --features metal
```

### Tests Unitaires & Intégration
Le système dispose d'une suite de tests robustes utilisant `AgentDbSandbox` pour garantir l'isolation totale des données de test.
```bash
# Tester spécifiquement le module génétique
cargo test genetics::handler::tests
```

## 📜 Traçabilité & Assurance (XAI)
Chaque décision prise par le cœur génère une **XaiFrame**. Ces trames documentent la méthode utilisée (ex: NSGA-II), les entrées (snapshot du graphe) et les métadonnées de performance (nombre de générations, convergence) pour assurer une auditabilité complète.

---
*RAISE Core — Engineering Intelligence for the Sovereignty Era.*

 

 