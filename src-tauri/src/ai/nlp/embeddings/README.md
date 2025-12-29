# 🧠 NLP Embeddings Engine

Ce module gère la vectorisation de texte (Text Embedding), brique fondamentale du système RAG (Retrieval-Augmented Generation) de GenAptitude. Il transforme le langage naturel en vecteurs mathématiques comparables.

## 🏗 Architecture

Le moteur utilise un **Pattern Stratégie** pour abstraire l'implémentation sous-jacente. L'interface publique est fournie par `EmbeddingEngine` dans `mod.rs`.

### Moteurs Disponibles

Nous supportons deux backends d'inférence, sélectionnables via l'enum `EngineType` :

#### 1. FastEmbed (Défaut)

- **Fichier** : `fast.rs`
- **Technologie** : Runtime ONNX (via la crate `fastembed`).
- **Modèle** : `BAAI/bge-small-en-v1.5`.
- **Avantages** : Très rapide, optimisé, téléchargement automatique des poids (quantized).
- **Usage** : Recommandé pour le développement et la production standard.

#### 2. Candle (Pure Rust)

- **Fichier** : `candle.rs`
- **Technologie** : Framework ML natif Rust de Hugging Face (`candle-core`, `candle-transformers`).
- **Modèle** : `sentence-transformers/all-MiniLM-L6-v2`.
- **Avantages** : Aucune dépendance système (pas de libonnx, pas de C++), idéal pour la compilation croisée ou les environnements restreints.
- **Fonctionnement** : Télécharge les poids `.safetensors` via `hf-hub`, tokenize, et exécute le graphe BERT manuellement.

## 📂 Structure des Fichiers

```text
src-tauri/src/ai/nlp/embeddings/
├── mod.rs       # Façade publique et dispatcher.
├── fast.rs      # Implémentation ONNX (FastEmbed).
└── candle.rs    # Implémentation Pure Rust (Candle/BERT).

```

## 🚀 Utilisation

```rust
use crate::ai::nlp::embeddings::{EmbeddingEngine, EngineType};

async fn example() -> Result<()> {
    // 1. Initialisation (Télécharge les modèles au premier lancement)
    // Par défaut (FastEmbed) :
    let mut engine = EmbeddingEngine::new()?;

    // Ou spécifiquement Candle :
    // let mut engine = EmbeddingEngine::new_with_type(EngineType::Candle)?;

    // 2. Vectorisation d'une requête (pour la recherche)
    let query_vec = engine.embed_query("Comment créer un acteur logique ?")?;
    println!("Vecteur de dimension : {}", query_vec.len()); // ex: 384

    // 3. Vectorisation par lot (pour l'indexation)
    let docs = vec![
        "L'ingénierie système est complexe.".to_string(),
        "Arcadia définit 5 couches.".to_string()
    ];
    let batch_vecs = engine.embed_batch(docs)?;

    Ok(())
}

```

## 📦 Gestion du Cache

Les modèles sont téléchargés automatiquement lors de la première exécution :

- **FastEmbed** : Stocké dans `src-tauri/.fastembed_cache/` (à exclure de Git).
- **Candle** : Stocké dans le cache standard Hugging Face (`~/.cache/huggingface/hub`).

## ⚠️ Notes Techniques

- **Mutabilité** : Les méthodes `embed_batch` et `embed_query` prennent `&mut self` car certains runtimes internes (ou tokenizers) peuvent nécessiter une mutabilité pour le cache interne ou les buffers.
- **Normalisation** : Les vecteurs de sortie sont normalisés (L2 Norm), ce qui permet d'utiliser le _Cosine Similarity_ via un simple produit scalaire (Dot Product).

```

---

```
