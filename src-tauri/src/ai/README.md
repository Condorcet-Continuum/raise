# Module AI — Intelligence Artificielle Neuro-Symbolique

Ce module implémente l'approche **MBAIE** (Model-Based AI Engineering) de GenAptitude. Il transforme le langage naturel en structures d'ingénierie formelles, valides et persistées.

## 🎯 Vision & Philosophie

L'IA de GenAptitude n'est pas un simple chatbot. C'est un **opérateur qualifié** qui agit sur le modèle.

1.  **Workstation-First** : Par défaut, l'intelligence tourne localement (Mistral via Docker).
2.  **Dual Mode** : Capacité à déborder sur le Cloud (Gemini Pro) pour les tâches complexes nécessitant un raisonnement supérieur.
3.  **Grounding (Ancrage)** : L'IA ne répond jamais "dans le vide". Elle est nourrie par le contexte réel du projet (`json_db`) via un système RAG.
4.  **Intégrité** : Les actions de l'IA passent par les mêmes validateurs (`x_compute`, Schema Validator) que les actions humaines.

---

## 🏗️ Architecture Modulaire

Le module est divisé en trois sous-systèmes interconnectés. Chaque sous-système possède sa propre documentation détaillée.

### 1\. [Le Cerveau Exécutif (`agents/`)](https://www.google.com/search?q=./agents/README.md)

Responsable de la compréhension et de l'action.

- **Intent Classifier** : Analyse la demande (ex: "Crée un acteur") et produit une structure Rust stricte.
- **Agents Spécialisés** :
  - `SystemAgent` : Crée/Modifie les éléments OA/SA (Acteurs, Fonctions).
  - _(Futur)_ `SoftwareAgent`, `HardwareAgent`.
- **Capacités** : Enrichissement automatique des données (description générée) et insertion en base.

### 2\. [La Mémoire Contextuelle (`context/`)](https://www.google.com/search?q=./context/README.md)

Responsable de l'ancrage des réponses dans la réalité du projet.

- **RAG Naïf (In-Memory)** : Le `SimpleRetriever` scanne le modèle chargé en RAM pour trouver les éléments pertinents liés à la question.
- **Injection** : Fournit au LLM un résumé textuel de l'existant ("Voici les acteurs actuels : ...").

### 3\. [L'Infrastructure d'Inférence (`llm/`)](https://www.google.com/search?q=./llm/README.md)

Responsable de la communication brute avec les modèles.

- **Client Dual Mode** : Interface unifiée `ask()` qui route vers :
  - **Local** : `http://localhost:8080` (Docker/Mistral).
  - **Cloud** : Google Vertex AI (Gemini Pro).
- **Robustesse** : Gestion des timeouts, ping de santé, parsing JSON résilient.

---

## 🔄 Flux de Données (Orchestration)

L'orchestration est gérée par la commande `ai_chat` (dans `commands/ai_commands.rs`) ou par le CLI (`tools/ai_cli`).

```mermaid
graph TD
    User[Utilisateur] -->|Input| Orch[Orchestrateur (Command/CLI)]

    subgraph "Phase 1 : Compréhension"
        Orch -->|Classify| AG[Agents / Intent]
        AG -->|JSON Mode| LLM[LLM]
        LLM -->|Intent| AG
    end

    subgraph "Phase 2 : Contexte (Si Chat)"
        Orch -->|Load Model| DB[(JSON-DB)]
        DB --> CTX[Context / Retriever]
        CTX -->|Snippet| LLM
    end

    subgraph "Phase 3 : Action (Si Création)"
        Orch -->|Process| AG
        AG -->|Generate Desc| LLM
        AG -->|Insert| DB
    end

    AG -->|Résultat| Orch
    Orch -->|Réponse| User
```

---

## 🛠️ Points d'Entrée

### 1\. Application GUI (Tauri)

L'utilisateur final interagit via le panneau de chat React.

- **Commande** : `ai_chat` (Async).
- **Retour** : Flux textuel ou confirmation d'action.

### 2\. Outil Développeur (`ai_cli`)

Pour le test rapide, l'automatisation et le débogage sans interface graphique.

- **Localisation** : `src-tauri/tools/ai_cli`.
- **Commandes** :
  - `chat` : Discussion libre avec contexte.
  - `classify -x` : Test de la chaîne d'exécution complète (Création DB).

---

## 📊 État d'Avancement (v0.1.0)

| Composant          | Statut     | Description                                         |
| :----------------- | :--------- | :-------------------------------------------------- |
| **LLM Client**     | ✅ Stable  | Support Local/Cloud, Gestion d'erreurs.             |
| **Classification** | ✅ Stable  | Détection précise (Create vs Chat), Nettoyage JSON. |
| **RAG**            | ⚠️ Basique | Recherche par mots-clés sur modèle en mémoire.      |
| **System Agent**   | ✅ Actif   | Création d'éléments OA/SA, Descriptions auto.       |
| **Software Agent** | ❌ Prévu   | Génération de code et composants logiques.          |
| **Vector DB**      | ❌ Prévu   | Remplacement du RAG naïf par Qdrant/LEANN.          |

---

> **Note aux contributeurs :**
> Pour modifier la logique d'un agent, voir `src/ai/agents`.
> Pour changer de modèle LLM, modifier le `.env`.
> Pour toucher à la base de données, passer par `json_db::collections::manager`.
