# Module Schema (Validation Structurelle)

Ce module implémente un moteur de validation JSON Schema "léger" et intégré, spécifiquement conçu pour l'architecture de RAISE. Il ne dépend pas de validateurs externes lourds et gère nativement la résolution de références internes (`$ref`) via un registre en mémoire.

## 🏗️ Architecture

Le système repose sur deux composants principaux :

1.  **`SchemaRegistry`** (`registry.rs`) : Charge et indexe tous les schémas disponibles dans une base de données (`.../schemas/v1/`). Il attribue à chaque fichier une URI unique de type `db://space/db/schemas/v1/...`.
2.  **`SchemaValidator`** (`validator.rs`) : Effectue la validation récursive d'un document JSON par rapport à un schéma racine chargé depuis le registre. Il supporte les références (`$ref`), les types (`object`, `string`...), les propriétés requises et les motifs (`patternProperties`).

## 🚀 Fonctionnalités Clés

### 1\. Registre de Schémas (`registry.rs`)

Le registre est l'autorité centrale des types. Au démarrage ou à la demande :

- Il scanne récursivement le dossier `schemas/v1` de la base de données.
- Il construit une map `URI -> Schema JSON`.
- Il fournit une méthode `uri("relative/path.json")` pour résoudre facilement les chemins.

### 2\. Validation (`validator.rs`)

Le validateur implémente une sous-partie stricte de la spécification JSON Schema Draft 2020-12, adaptée aux besoins d'Arcadia.

- **Types** : Vérification des types primitifs (`string`, `number`, `boolean`, `array`, `object`, `null`).
- **Objets** :
  - `required` : Vérifie la présence des champs obligatoires.
  - `properties` : Valide récursivement les sous-objets.
  - `patternProperties` : Valide les clés dynamiques via Regex (ex: `^x_` pour les extensions).
  - `additionalProperties` : Si `false`, rejette toute clé non définie (sauf `$schema` toléré).
- **Références (`$ref`)** : Résolution automatique des pointeurs JSON internes (`#/...`) et des fichiers externes (`other.schema.json`) via le registre.

### 3\. Cycle de Vie

La méthode `compute_then_validate` est un vestige de l'ancienne architecture. Aujourd'hui, elle sert de point d'entrée simple vers `validate`. Les calculs (valeurs par défaut, IDs, dates) sont désormais gérés en amont par le **Rules Engine** (`manager.rs`) avant que le document n'arrive ici.

## 🛠️ Utilisation

```rust
use crate::json_db::schema::{SchemaRegistry, SchemaValidator};
use serde_json::json;

// 1. Initialiser le registre (charge tous les schémas du dossier)
let registry = SchemaRegistry::from_db(&config, "my_space", "my_db")?;

// 2. Préparer un validateur pour un type précis
let root_uri = registry.uri("actors/actor.schema.json");
let validator = SchemaValidator::compile_with_registry(&root_uri, &registry)?;

// 3. Valider un document
let doc = json!({
    "id": "123",
    "name": "Alice"
});

match validator.validate(&doc) {
    Ok(_) => println!("Document valide !"),
    Err(e) => println!("Erreur de validation : {}", e),
}
```

## 📂 Structure des Fichiers

```text
src-tauri/src/json_db/schema/
├── mod.rs          // Exports et définitions d'erreurs
├── registry.rs     // Chargement et indexation des fichiers .schema.json
└── validator.rs    // Moteur de validation récursif (types, refs, regex)
```

## ⚠️ Limitations

- **Keywords Supportés** : Seuls `type`, `properties`, `required`, `patternProperties`, `additionalProperties`, `$ref` sont pleinement supportés. Des mots-clés avancés comme `oneOf`, `anyOf`, `if/then/else` (au niveau structurel) ne sont pas implémentés dans ce validateur léger.
- **Performance** : Le registre charge tous les schémas en mémoire. Pour des milliers de schémas, une stratégie de chargement paresseux (Lazy Loading) pourrait être nécessaire.
