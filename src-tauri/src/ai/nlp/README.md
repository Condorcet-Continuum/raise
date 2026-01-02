# Module `ai/nlp` — Traitement du Langage Naturel

Ce module regroupe les outils de **bas niveau** pour la manipulation technique du texte. Contrairement au module `agents` qui gère le sens (sémantique), le module `nlp` gère la forme (syntaxe, tokens, vecteurs).

Il sert de bibliothèque utilitaire transversale pour `llm` (gestion du contexte) et `context` (préparation des données RAG).

## 🎯 Objectifs

1.  **Tokenization** : Transformer le texte brut en tokens pour estimer la taille des prompts et éviter de dépasser la fenêtre de contexte des modèles (ex: 4096 tokens pour Mistral).
2.  **Chunking (Découpage)** : Diviser intelligemment les documents longs en morceaux digestes pour le RAG.
3.  **Vectorisation (Embeddings)** : Transformer le texte en vecteurs mathématiques (`Vec<f32>`) pour la recherche sémantique (via Qdrant/LEANN).

---

## 📂 Architecture Prévue

```mermaid
graph TD
    Input[Texte Brut] -->|Tokenizer| Tokens[Liste de Tokens]
    Tokens -->|Counter| Cost[Estimation Coût/Taille]

    Input -->|Splitter| Chunks[Fragments de Texte]
    Chunks -->|Embedder| Vectors[Vecteurs (Float32)]

    Vectors --> VectorDB[(Vector DB / Qdrant)]
```

### 1\. `tokenizers` _(À implémenter)_

Wrapper autour de la crate Rust `tokenizers` (HuggingFace).

- **Usage** : Avant d'envoyer une requête à `LlmClient`, on vérifie : `if count_tokens(prompt) > 4000 { error("Prompt trop long") }`.
- **Modèles supportés** : BPE (Byte-Pair Encoding) compatible Llama/Mistral.

### 2\. `splitting` _(À implémenter)_

Algorithmes de découpage de texte.

- **Naïf** : Découpage par caractères (ex: tous les 1000 chars).
- **Sémantique** : Découpage respectant les paragraphes (Markdown headers, sauts de ligne) pour ne pas couper une phrase en deux.
- **Overlap** : Gestion du chevauchement (ex: 10% de recouvrement entre deux chunks) pour préserver le contexte aux frontières.

### 3\. `embeddings` _(À implémenter)_

Interface pour générer des vecteurs.

- **Local** : Utilisation de `ort` (ONNX Runtime) avec un petit modèle type `all-MiniLM-L6-v2` (\~80MB) embarqué dans l'app.
- **Cloud** : Appel à l'API Embeddings de Google/OpenAI (si mode Cloud activé).

---

## 🔄 Intégration dans le flux

### Flux actuel (v0.1.0)

Le module est passif. Le découpage est fait sommairement dans `ai/context/retriever.rs`.

### Flux cible (v0.2.0)

1.  **L'Agent** génère un prompt.
2.  **NLP** calcule les tokens : "Attention, il ne reste que 500 tokens pour la réponse".
3.  **Context** récupère un gros fichier de documentation.
4.  **NLP** le découpe en chunks de 512 tokens.
5.  **NLP** vectorise ces chunks.
6.  **Context** cherche les 3 chunks les plus proches mathématiquement de la question utilisateur.

---

## 🛠️ Stack Technique envisagée

- **Crate `tokenizers`** : Standard industriel, écrit en Rust, très rapide.
- **Crate `text-splitter`** : Pour le chunking intelligent.
- **Crate `candle-core`** ou **`ort`** : Pour faire tourner des modèles d'embedding (BERT/MiniLM) directement en Rust sans Python.

---

## 📊 État d'Avancement

| Composant             | Statut     | Priorité                          |
| :-------------------- | :--------- | :-------------------------------- |
| **Token Counter**     | ❌ À faire | Haute (pour robustesse LLM)       |
| **Markdown Splitter** | ❌ À faire | Moyenne (pour RAG avancé)         |
| **ONNX Embedder**     | ❌ À faire | Basse (pour recherche sémantique) |

---

> **Note :** Ce module est pour l'instant une coquille architecturale destinée à accueillir la complexité croissante du traitement de texte au fur et à mesure que RAISE montera en puissance.
