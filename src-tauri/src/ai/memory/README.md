# Module Memory — Mémoire Long-Terme & Vectorielle

Ce module gère la **persistance sémantique** de l'IA. Il agit comme l'hippocampe du système RAISE : il stocke les informations (documents, notes, fragments de modèle) sous forme vectorielle pour permettre une recherche par le sens (Sémantique) plutôt que par mot-clé exact.

---

## 🏗️ Architecture Technique

Le module repose sur le **Pattern Strategy** pour découpler l'application du moteur de base de données sous-jacent.

### 1. L'Abstraction (`mod.rs`)

Nous définissons une interface générique `VectorStore` que tout backend doit implémenter. Cela permettrait, théoriquement, de passer de Qdrant à PgVector ou Milvus sans casser le reste du code.

- **`MemoryRecord`** : La structure de donnée standard. Contient l'ID, le texte brut, les métadonnées JSON et le vecteur (embedding).
- **`VectorStore` (Trait)** : Définit les opérations atomiques : `init_collection`, `add_documents`, `search_similarity`.

### 2. L'Implémentation (`qdrant_store.rs`)

L'implémentation actuelle utilise **Qdrant**, une base de données vectorielle performante écrite en Rust.

- **Protocole** : gRPC (Port 6334) pour une performance maximale.
- **Payload** : Les métadonnées et le contenu textuel sont stockés dans le payload JSON de Qdrant.
- **Conversion** : Gère la sérialisation complexe entre les types Rust natifs et les types Protobuf de Qdrant.

---

## 🛠️ Prérequis Infrastructure

Ce module nécessite une instance Qdrant active. Dans l'environnement de développement RAISE, cela est géré par Docker.

```bash
# Lancer l'infrastructure (à la racine du projet)
docker-compose up -d

```

| Service         | Port Interne (Docker) | Port Hôte (Localhost) | Usage                                         |
| --------------- | --------------------- | --------------------- | --------------------------------------------- |
| **Qdrant gRPC** | 6334                  | **6334**              | Ingestion & Recherche (Utilisé par ce module) |
| **Qdrant HTTP** | 6333                  | **6333**              | Dashboard & API REST                          |

---

## 💻 Utilisation dans le Code

Ce module est rarement utilisé seul. Il est généralement orchestré par le module `ai::context::rag`. Cependant, voici comment l'utiliser bas niveau :

```rust
use crate::ai::memory::{qdrant_store::QdrantMemory, MemoryRecord, VectorStore};
use serde_json::json;

async fn example_usage() -> anyhow::Result<()> {
    // 1. Connexion
    let store = QdrantMemory::new("http://localhost:6334")?;

    // 2. Initialisation (Si la collection n'existe pas)
    // 384 est la taille standard pour le modèle 'BGE-Small'
    store.init_collection("ma_base_connaissance", 384).await?;

    // 3. Insertion
    let doc = MemoryRecord {
        id: uuid::Uuid::new_v4().to_string(),
        content: "La spec ISO-26262 traite de la sécurité fonctionnelle.".to_string(),
        metadata: json!({"source": "specs", "page": 42}),
        vectors: Some(vec![0.1, 0.5, ...]), // Vecteur généré par le module NLP
    };
    store.add_documents("ma_base_connaissance", vec![doc]).await?;

    // 4. Recherche
    let query_vector = vec![0.1, 0.5, ...];
    let results = store.search_similarity("ma_base_connaissance", &query_vector, 5, 0.7).await?;

    Ok(())
}

```

---

## 🧪 Tests & Validation

Le module contient un test d'intégration (`tests.rs`) qui vérifie le cycle de vie complet : Connexion -> Création Collection -> Insertion -> Recherche.

**Note :** Docker doit être lancé pour que ces tests passent.

```bash
# Lancer uniquement les tests de ce module
cargo test --package raise --lib -- ai::memory::tests

```

### Scénario de Test

1. Crée une collection temporaire `test_memory_suite`.
2. Insère deux vecteurs orthogonaux (ex: "Nord" et "Est").
3. Effectue une recherche proche de l'un des vecteurs.
4. Vérifie que le bon document est retrouvé et que les métadonnées sont intactes.

---

## 📂 Structure des Fichiers

```text
src-tauri/src/ai/memory/
├── mod.rs            # Définition des Traits et Structs (Interface)
├── qdrant_store.rs   # Driver Qdrant (Implémentation)
├── tests.rs          # Tests d'intégration (requires Docker)
└── README.md         # Ce fichier

```
