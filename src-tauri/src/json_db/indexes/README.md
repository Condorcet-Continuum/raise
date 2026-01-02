# Module Indexes (JSON-DB)

Ce module fournit un système d'indexation performant et extensible pour les collections JSON de RAISE. Il permet d'accélérer les requêtes, d'imposer des contraintes d'unicité et de supporter la recherche plein texte rudimentaire.

## 🏗️ Architecture

L'architecture repose sur une séparation claire entre la gestion de haut niveau (`manager`), l'implémentation spécifique des types d'index (`hash`, `btree`, `text`) et le stockage bas niveau (`driver`).

### Composants Clés

- **`manager.rs` (IndexManager)** : Le point d'entrée principal. Il orchestre la création, la suppression, la reconstruction (backfill) et la mise à jour des index lors des écritures de documents. Il maintient les métadonnées dans `_meta.json`.
- **`driver.rs` (Generic Driver)** : Fournit une abstraction I/O unifiée pour charger, modifier et sauvegarder les index sur disque. Il utilise le format binaire **Bincode** pour la performance et implémente le trait `IndexMap` pour supporter indifféremment `HashMap` (Hash) et `BTreeMap` (B-Tree).
- **Implémentations Spécifiques** :
  - **`hash.rs`** : Index de hachage standard pour les égalités exactes (`IndexType::Hash`). Utilise `HashMap<String, Vec<String>>`.
  - **`btree.rs`** : Index ordonné pour les recherches par plage (`IndexType::BTree`). Utilise `BTreeMap<String, Vec<String>>`.
  - **`text.rs`** : Index inversé pour la recherche textuelle simple (`IndexType::Text`). Tokenise le texte en minuscules alphanumériques.

## 📂 Stockage sur Disque

Les index sont stockés dans un sous-dossier `_indexes` au sein de chaque collection.

- **Chemin** : `{db_root}/{collection}/_indexes/{field_name}.{type}.idx`
- **Format** : Binaire sérialisé (Bincode 2.0 standard configuration).
- **Structure** : Liste de `IndexRecord` `{ key: String, document_id: String }`. Notez que la clé est stockée sous forme de chaîne JSON brute pour éviter les problèmes de désérialisation dynamique `serde_json::Value` avec Bincode.

## 🚀 Fonctionnalités

### 1\. Types d'Index Supportés

| Type      | Usage                              | Structure Interne   | Complexité (Insert/Search) |
| :-------- | :--------------------------------- | :------------------ | :------------------------- |
| **Hash**  | Recherche exacte (`=`), Unicité    | `HashMap`           | O(1) moyen                 |
| **BTree** | Tri, Plages (`<`, `>`, `<=`, `>=`) | `BTreeMap`          | O(log n)                   |
| **Text**  | Recherche de mots-clés             | `HashMap` (Inversé) | O(1) par token             |

### 2\. Gestion du Cycle de Vie (`IndexManager`)

- **Création (`create_index`)** :
  1.  Valide le type d'index.
  2.  Ajoute la définition dans `_meta.json`.
  3.  Lance immédiatement un **Rebuild (Backfill)** : parcourt tous les documents JSON existants de la collection pour peupler le fichier d'index.
- **Mise à jour (`index_document` / `remove_document`)** :
  - Appelé par le `CollectionsManager` lors de chaque écriture.
  - Charge les définitions d'index actives.
  - Calcule le diff entre l'ancienne et la nouvelle valeur du champ indexé.
  - Met à jour atomiquement le fichier d'index correspondant.
- **Suppression (`drop_index`)** :
  - Supprime la définition de `_meta.json`.
  - Supprime physiquement le fichier `.idx` sur le disque.

### 3\. Contrainte d'Unicité

Le driver générique supporte nativement la contrainte `unique: true`. Lors d'une insertion, si la clé existe déjà et pointe vers un autre ID de document, une erreur `Index unique constraint violation` est levée, empêchant l'opération d'écriture globale.

## 🛠️ Utilisation (Interne)

Ce module est principalement utilisé par `CollectionsManager` et le moteur de requêtes.

```rust
// Exemple d'utilisation via IndexManager
let mut idx_mgr = IndexManager::new(storage, "space", "db");

// Indexer un nouveau document
idx_mgr.index_document("users", &doc_json)?;

// Créer un nouvel index sur le champ "email" (déclenche un backfill)
idx_mgr.create_index("users", "email", "hash")?;
```

## ⚠️ Notes Techniques

- **Pointeurs JSON** : Les champs à indexer sont définis par des pointeurs JSON (ex: `/address/city`). Si le champ est imbriqué, le chemin doit être complet.
- **Tokenisation Textuelle** : L'index textuel utilise un tokenizer simple qui ne garde que les caractères alphanumériques et convertit tout en minuscules. Il ne supporte pas (encore) le stemming ou les stop-words avancés.
- **Performance** : Les fichiers d'index sont chargés intégralement en mémoire lors des mises à jour. Pour de très gros index, une implémentation B-Tree sur disque (type SQLite ou pages binaires) serait une future évolution nécessaire.
