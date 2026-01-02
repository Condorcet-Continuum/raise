# Module Storage (JSON-DB)

Ce module implémente la couche physique de stockage de données pour RAISE. Il gère l'interaction directe avec le système de fichiers, l'organisation des répertoires, le cache mémoire, et l'atomicité des écritures.

## 🏗️ Architecture

Le moteur de stockage est organisé autour de plusieurs composants clés :

1.  **`StorageEngine`** (`mod.rs`) : La façade principale qui combine la persistance disque et le cache mémoire. C'est l'objet manipulé par les couches supérieures (`CollectionsManager`, `IndexManager`).
2.  **`file_storage`** (`file_storage.rs`) : Gère les opérations bas niveau sur le système de fichiers (lecture, écriture atomique, suppression, création de DB).
3.  **`cache`** (`cache.rs`) : Un cache LRU (Least Recently Used) thread-safe avec expiration (TTL) pour accélérer les lectures répétées.
4.  **`JsonDbConfig`** (`mod.rs`) : Définit la racine de stockage et les chemins standards vers les bases, collections et schémas.

## 🚀 Fonctionnalités Clés

### 1\. Organisation des Fichiers

Le stockage suit une structure hiérarchique stricte:

- **Racine** : `data_root/`
- **Espace** (Tenant/Projet) : `data_root/{space}/`
- **Base de Données** : `data_root/{space}/{db}/`
- **Collections** : `data_root/{space}/{db}/collections/{collection}/`
- **Documents** : `{id}.json` (un fichier par document).
- **Schémas Système** : `data_root/{space}/_system/schemas/v1/` (centralisés pour tous les DBs de l'espace).

### 2\. Écritures Atomiques

Pour garantir l'intégrité des données en cas de crash ou de coupure de courant, toutes les écritures (documents et index) sont **atomiques**.

- Le contenu est d'abord écrit dans un fichier temporaire (`.tmp`).
- Une fois l'écriture validée, le fichier temporaire est renommé (`fs::rename`) vers sa destination finale. Cette opération est garantie atomique par la plupart des systèmes de fichiers modernes (EXT4, NTFS, APFS).

### 3\. Cache Mémoire (LRU)

Le `StorageEngine` intègre un cache automatique pour les documents.

- **Lecture** : `read_document` vérifie d'abord le cache. Si absent, il lit le disque et peuple le cache.
- **Écriture** : `write_document` met à jour le fichier ET le cache simultanément.
- **Suppression** : `delete_document` supprime le fichier ET invalide l'entrée de cache.
- **Politique** : Le cache a une capacité fixe (ex: 1000 items) et un TTL optionnel. Il utilise une stratégie d'éviction LRU simple (supprime les entrées expirées ou les plus anciennes si plein).

### 4\. Déploiement des Schémas (Embedded)

Le module `file_storage` intègre une fonctionnalité cruciale : le déploiement automatique des schémas JSON par défaut.
Grâce à la crate `include_dir`, les fichiers du dossier `schemas/v1` sont compilés dans le binaire. Lors de la création d'une base (`create_db`), ces schémas sont extraits physiquement sur le disque si nécessaire, garantissant que l'application est toujours livrée avec ses définitions de structure à jour.

## 🛠️ Utilisation

```rust
use crate::json_db::storage::{JsonDbConfig, StorageEngine};

// 1. Configuration
let config = JsonDbConfig::new(PathBuf::from("/tmp/raise_data"));
let storage = StorageEngine::new(config);

// 2. Initialisation d'une DB (déploie les schémas)
storage.init_db("my_space", "my_db")?;

// 3. Écriture (disque + cache)
let doc = json!({ "id": "1", "name": "Test" });
storage.write_document("my_space", "my_db", "users", "1", &doc)?;

// 4. Lecture (cache first)
let read_doc = storage.read_document("my_space", "my_db", "users", "1")?;
```

## 📂 Structure des Fichiers

```text
src-tauri/src/json_db/storage/
├── mod.rs          // Façade StorageEngine et Configuration
├── file_storage.rs // Opérations I/O bas niveau (fs::write, include_dir)
├── cache.rs        // Implémentation du Cache LRU thread-safe
└── compression.rs  // (Placeholder) Future implémentation de la compression
```

## ⚠️ Notes Techniques

- **Verrouillage** : Le `StorageEngine` n'implémente pas de verrouillage de fichier (file locking). Il suppose que l'application Tauri est le seul processus accédant à ces fichiers (Single Writer).
- **Performance** : Pour des collections massives (\> 100k fichiers), le système de fichiers peut devenir un goulot d'étranglement (inodes, listage de répertoire). Une stratégie de sharding (sous-dossiers) pourrait être envisagée à l'avenir.
