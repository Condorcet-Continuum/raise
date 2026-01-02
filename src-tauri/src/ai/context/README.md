# Module Context — Mémoire & Ancrage (RAG Hybride)

Ce module est le garant de la **Vérité Terrain** (Grounding) de l'IA. Il est responsable de fournir au LLM le contexte nécessaire pour répondre aux questions de l'ingénieur, en combinant connaissances techniques, état du modèle et historique de la conversation.

---

## 🏗️ Architecture Globale (The 4-Pillars)

Le contexte de RAISE repose sur 4 piliers distincts pour couvrir tous les horizons temporels :

| Composant       | Fichier                   | Type de Mémoire           | Objectif                                                | Exemple                           |
| --------------- | ------------------------- | ------------------------- | ------------------------------------------------------- | --------------------------------- |
| **Symbolique**  | `retriever.rs`            | **Immédiate** (RAM)       | Scanner le modèle structuré actuel (`ProjectModel`).    | _"Liste les acteurs définis."_    |
| **Sémantique**  | `rag.rs`                  | **Long-Terme** (Vector)   | Chercher dans la documentation/notes (Qdrant).          | _"C'est quoi la norme ISO-123 ?"_ |
| **Session**     | `conversation_manager.rs` | **Court-Terme** (Working) | Gérer le fil de discussion et le contexte glissant.     | _"Modifie-le."_ (Qui est "le" ?)  |
| **Persistance** | `memory_store.rs`         | **Stockage** (File/KV)    | Sauvegarder/Charger les historiques de chat sur disque. | _Reprendre une discussion hier._  |

---

## 🔄 Flux de Données (Data Flow)

Ce diagramme illustre comment la **Mémoire de Travail** (Conversation) interagit avec la **Mémoire de Recherche** (Retrievers) pour former le contexte final.

```text
                               QUESTION UTILISATEUR
                                       |
                                       v
                           [ CONVERSATION MANAGER ]
                                       |
                   +-------------------+-------------------+
                   | (Gestion de l'historique & Sliding Window)
                   v
           [ MEMORY STORE ] (Load/Save History JSON)
                   |
                   v
        "Question Contextualisée" (ex: "Modifie-le" -> "Modifie le Moteur")
                   |
                   v
             [ ORCHESTRATOR ] ------------------------+
                   |                                  |
         (Voie Déterministe)                  (Voie Probabiliste)
                   |                                  |
         [ SimpleRetriever ]                  [ RagRetriever ]
                   |                                  |
      1. Scan Mots-clés (RAM)               1. Vectorisation (FastEmbed)
      2. Filtre Structuré                   2. Recherche Qdrant (Docker)
                   |                                  |
                   v                                  v
        [ Éléments du Modèle ]               [ Chunks de Documentation ]
                   |                                  |
                   +----------------+-----------------+
                                    |
                                    v
                           [ CONTEXT BUILDER ]
                    (Fusion : Historique + Modèle + Docs)
                                    |
                                    v
                             [ LLM CLIENT ]

```

---

## 📂 Organisation du Code

```text
src-tauri/src/ai/context/
├── mod.rs                   # Point d'entrée
├── retriever.rs             # Moteur Symbolique (Scan du Modèle structuré)
├── rag.rs                   # Moteur Sémantique (Client Qdrant + Embeddings)
├── conversation_manager.rs  # Gestionnaire de session (Historique, Token limit)
├── memory_store.rs          # Persistance locale des conversations
└── tests/                   # Tests unitaires et d'intégration

```

---

## 🧠 1. Le Moteur Symbolique (`retriever.rs`)

_Approche "Exacte"_.
Parcourt les structures Rust en mémoire (`ProjectModel`) pour trouver des correspondances exactes de noms ou de descriptions. Indispensable pour que l'IA manipule les bons objets du diagramme.

## 🔮 2. Le Moteur Sémantique (`rag.rs`)

_Approche "Conceptuelle"_.
Utilise **Qdrant** et **FastEmbed** pour retrouver des informations dans des textes non structurés (spécifications, wiki projet) en se basant sur le sens (vecteurs) plutôt que sur les mots exacts.

## 🗣️ 3. Le Gestionnaire de Session (`conversation_manager.rs`)

_Mémoire de Travail_.
L'IA n'a pas de mémoire native d'une requête à l'autre. Ce module :

- Stocke les échanges `User` <-> `Assistant`.
- Applique une fenêtre glissante (ex: garde les 10 derniers échanges) pour ne pas saturer le contexte du LLM.
- Résout les références anaphoriques (transformer "il" ou "ça" en l'objet mentionné précédemment).

## 💾 4. Le Stockage de Mémoire (`memory_store.rs`)

_Persistance_.
Assure que les conversations ne sont pas perdues au redémarrage de l'application. Il sérialise l'état du `ConversationManager` vers le système de fichiers (JSON ou Bincode).

---

## 🚀 Commandes de Test

### Tester le Retriever Symbolique

```bash
cargo test context::tests

```

### Tester le Pipeline RAG Complet

```bash
cargo test rag_integration_test

```

---

## 🛠️ État d'avancement & Roadmap

- [x] **Retriever Symbolique** : Fonctionnel (Recherche par mots-clés).
- [x] **RAG Sémantique** : Fonctionnel (Connexion Qdrant + FastEmbed).
- [ ] **Conversation Manager** : À implémenter (Structure de données `ChatHistory`).
- [ ] **Memory Store** : À implémenter (Sauvegarde JSON locale dans `.raise/chats/`).
- [ ] **Orchestrateur Unifié** : Fusionner les 4 sources avant l'envoi au LLM.
