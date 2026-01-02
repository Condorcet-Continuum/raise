# RAISE · Usine de Cas d'Usage IA Orientée Poste de Travail

<p align="center">
  <img src="../../src/assets/images/logo-white.svg" alt="RAISE Logo" width="200">
</p>

**RAISE** est une plateforme d'ingénierie système (MBSE) souveraine et locale. Elle permet de transformer des tâches d'ingénierie complexes en assistants **locaux, auditables et explicables** en combinant IA générative et modélisation formelle.

Ce projet est un monorepo contenant une **application de bureau (Tauri v2 + Rust)**, une **interface réactive (React + Vite)**, un noyau de calcul en **WebAssembly**, et un moteur de **base de données JSON transactionnelle**.

---

## 🚀 Fonctionnalités Techniques Clés

### 🧠 MBAIE (Model-Based AI Neuro-Symbolic Engineering)

RAISE implémente une approche hybride :

- [cite_start]**Orchestration Multi-Agents** : Agents spécialisés (`Software`, `System`, `Hardware`) pilotés par des modèles formels Arcadia/Capella[cite: 12].
- [cite_start]**Contexte Sémantique** : Support natif de **JSON-LD** (`json_db/jsonld`) pour lier les données aux ontologies métiers (OA, SA, LA, PA, EPBS)[cite: 5].
- [cite_start]**Inférence Locale** : Architecture conçue pour fonctionner avec des LLMs locaux (via `llama.cpp`) et une mémoire vectorielle (RAG) sans dépendance cloud[cite: 52].

### 📦 JSON-DB Transactionnelle

Un moteur de base de données NoSQL sur-mesure développé en Rust (`src-tauri/src/json_db`) :

- [cite_start]**Stockage Local** : Données stockées en fichiers JSON, validées par **JSON Schema** avant écriture[cite: 13].
- [cite_start]**Transactions ACID** : Support complet des transactions multi-documents grâce à un **Write-Ahead Log (WAL)** (`_wal.jsonl`) garantissant l'atomicité[cite: 636, 638].
- [cite_start]**Moteur `x_compute`** : Calcul automatique de champs (UUID, timestamps, agrégats) intégré au pipeline d'insertion[cite: 969].
- **Indexation** : Index Hash, BTree et Textuels maintenus en mémoire pour des performances de lecture élevées.

### 🛡️ Souveraineté & Réseau Mesh

- [cite_start]**Blockchain Fabric** : Client gRPC intégré (`blockchain/fabric`) pour l'enregistrement immuable des décisions d'architecture sur Hyperledger Fabric.
- [cite_start]**VPN Mesh (Innernet)** : Client WireGuard embarqué (`blockchain/vpn`) pour créer des réseaux privés sécurisés (Interface `raise0`) entre postes ingénieurs.
- **Traçabilité** : Audit trail complet pour la conformité aux standards critiques (DO-178C, ISO-26262)[cite: 16].

---

## 🛠️ Installation et Démarrage

### Prérequis

- **Node.js 20+** (Gestion du frontend)
- **Rust 1.88+** (Backend et WASM)
- [cite_start]**Cibles WASM** : `rustup target add wasm32-unknown-unknown wasm32-wasip1`[cite: 34].

### Commandes Rapides

1.  **Compiler le module WASM** (Requis pour le fonctionnement de l'UI) :

    ```bash
    cd src-wasm && ./build.sh && cd ..
    ```

2.  **Lancer l'environnement de développement** :

    ```bash
    npm install
    cargo tauri dev
    ```

    Ceci lancera simultanément le serveur Vite (Frontend) et le backend Tauri.

3.  **Administration BDD (CLI)** :
    Pour interagir avec la base de données sans l'interface graphique :
    ```bash
    cd src-tauri/tools/jsondb_cli
    # Exemple : Lister tous les documents d'une collection
    cargo run -- query find-many un2 _system query.json
    ```

---

## 🏗️ Structure du Projet

- **`src-tauri/`** : Backend Rust. Cœur de l'application.
  - `json_db/` : Moteur de base de données custom (Collections, Index, WAL).
  - `blockchain/` : Clients Fabric (gRPC) et Innernet (WireGuard).
  - `ai/` : Orchestrateur, Agents et NLP.
  - `model_engine/` : Logique métier Arcadia/Capella.
- [cite_start]**`src-wasm/`** : Code Rust compilé en WebAssembly pour les calculs lourds côté client (Algorithmes de graphes, Parsing XMI)[cite: 39].
- **`src/`** : Frontend React/TypeScript (Composants, Stores Zustand, Services).
- [cite_start]**`schemas/`** : Définitions JSON Schema & JSON-LD versionnées (v1) pour tous les objets métier[cite: 5].
- **`domain-models/`** : Modèles de référence métier (Arcadia, HDL, Software Patterns)[cite: 1].

---

## Contact

**RAISE — Workstation-First AI Use-Case Factory**
Contact : **zair@bezghiche.com**
