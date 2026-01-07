# Module Memory — Mémoire Sémantique Hybride

Ce module gère la **persistance sémantique** de l'IA (Long-Term Memory). Il agit comme l'hippocampe du système RAISE en stockant les documents et contextes sous forme vectorielle (embeddings) pour permettre une recherche par le sens plutôt que par mot-clé exact.

---

## 🏗️ Architecture : Pattern Strategy

Le module est conçu autour d'une architecture flexible qui découple la logique métier du moteur de stockage sous-jacent.

### 1. L'Abstraction (`VectorStore`)

Nous définissons une interface générique (Trait) `VectorStore`. Tout moteur de base de données doit implémenter ces trois méthodes atomiques :

- `init_collection` : Prépare l'index ou la table.
- `add_documents` : Vectorise et stocke les données.
- `search_similarity` : Retrouve les documents les plus proches sémantiquement.

### 2. Les Moteurs (Backends)

Nous supportons actuellement deux implémentations distinctes selon les besoins de déploiement :

| Caractéristique | **Qdrant** (`qdrant_store.rs`) | **LEANN** (`leann_store.rs`)            |
| --------------- | ------------------------------ | --------------------------------------- |
| **Type**        | Serveur DB Autonome (Rust)     | Librairie/Service Python léger          |
| **Protocole**   | **gRPC** (Port 6334)           | **HTTP/REST** (Port 8000)               |
| **Performance** | Très Haute (Production)        | Moyenne (Optimisé Low-RAM)              |
| **Usage Idéal** | Serveur, Cloud, Gros volumes   | Local-first, Laptop, Embarqué           |
| **Dépendance**  | Image Docker Officielle        | Dockerfile Custom (Wrapper Rust/Python) |

---

## 🛠️ Installation & Infrastructure

L'infrastructure est gérée via Docker Compose.

### 1. Démarrer les services

Pour lancer la stack complète (Qdrant + LEANN) :

```bash
# L'option --build est nécessaire la première fois pour compiler le wrapper LEANN
docker-compose up -d --build

```

### 2. Configuration des Ports

Les ports sont configurables via le fichier `.env` ou `docker-compose.yml`:

| Service         | Port Défaut | Variable ENV       | Description                           |
| --------------- | ----------- | ------------------ | ------------------------------------- |
| **Qdrant gRPC** | 6334        | `PORT_QDRANT_GRPC` | Performance maximale pour l'ingestion |
| **Qdrant HTTP** | 6333        | `PORT_QDRANT_HTTP` | Dashboard UI de Qdrant                |
| **LEANN API**   | 8000        | `PORT_LEANN`       | API REST du wrapper Python/Rust       |

---

## 💻 Exemple d'Utilisation (Code)

Le choix du moteur se fait à l'instanciation. Le reste du code est agnostique grâce au trait `VectorStore`.

```rust
use crate::ai::memory::{
    qdrant_store::QdrantMemory,
    leann_store::LeannMemory,
    MemoryRecord, VectorStore
};
use serde_json::json;

async fn init_memory(engine: &str) -> anyhow::Result<Box<dyn VectorStore>> {
    let store: Box<dyn VectorStore> = match engine {
        "local" => {
            println!("🚀 Démarrage en mode LEANN (Léger)");
            Box::new(LeannMemory::new("http://localhost:8000")?)
        },
        _ => {
            println!("🚀 Démarrage en mode QDRANT (Production)");
            Box::new(QdrantMemory::new("http://localhost:6334")?)
        }
    };

    // Le reste du code est identique quel que soit le moteur !
    store.init_collection("ma_base", 384).await?;

    // Insertion
    let doc = MemoryRecord {
        id: uuid::Uuid::new_v4().to_string(),
        content: "L'architecture hexagonale permet de tester facilement.".to_string(),
        metadata: json!({"tag": "archi"}),
        vectors: Some(vec![0.1, 0.2, 0.3, 0.4]),
    };

    store.add_documents("ma_base", vec![doc]).await?;

    Ok(store)
}

```

---

## 🧪 Tests & Validation

Le module contient des tests d'intégration spécifiques pour chaque moteur.

> **⚠️ Prérequis :** Les conteneurs Docker (`raise_qdrant` et `raise_leann`) doivent être lancés avant de jouer les tests.

### Tester Qdrant

Vérifie la connexion gRPC et la persistance standard.

```bash
cargo test --package raise --lib -- test_qdrant_lifecycle --nocapture

```

### Tester LEANN

Vérifie la connexion HTTP et le wrapper Python.

```bash
cargo test --package raise --lib -- test_leann_lifecycle --nocapture --ignored

```

_(Note : Le flag `--ignored` est requis car ce test est désactivé par défaut pour la CI/CD rapide)._

---

## 📂 Structure des Fichiers

```text
src-tauri/src/ai/memory/
├── mod.rs            # Interface VectorStore & Structs communes
├── qdrant_store.rs   # Implémentation Client gRPC Qdrant
├── leann_store.rs    # Implémentation Client HTTP LEANN
├── tests.rs          # Tests d'intégration (Lifecycle Qdrant & LEANN)
└── README.md         # Documentation

```
