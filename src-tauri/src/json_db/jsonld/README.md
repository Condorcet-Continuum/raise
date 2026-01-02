# Module JSON-LD (Semantic Engine)

Ce module implémente une couche d'abstraction **Sémantique** pour RAISE. Il permet de traiter les documents JSON non seulement comme des objets structurés, mais comme des graphes de connaissances liés (Linked Data), conformes à l'ontologie Arcadia (OA, SA, LA, PA, EPBS).

## 🏗️ Architecture

Le module est articulé autour de trois composants majeurs :

1.  **`JsonLdProcessor`** (`processor.rs`) : Le moteur de traitement. Il offre les algorithmes standards JSON-LD (Expansion, Compaction) et la conversion vers RDF (N-Triples).
2.  **`ContextManager`** (`context.rs`) : Gère la résolution des préfixes (ex: `oa:` -\> `https://...`). Il maintient les mappages actifs entre les termes courts et les IRIs complets.
3.  **`VocabularyRegistry`** (`vocabulary.rs`) : Le "Dictionnaire" de l'application. Il contient la définition codée en dur de toutes les classes et propriétés valides de l'ontologie Arcadia.

## 🧠 Ontologie Arcadia

RAISE définit ses propres espaces de noms (Namespaces) pour mapper les concepts de la méthode Arcadia. Ces définitions se trouvent dans `vocabulary.rs`.

| Couche           | Préfixe | URI de Base         | Description                                    |
| :--------------- | :------ | :------------------ | :--------------------------------------------- |
| **Opérationnel** | `oa:`   | `.../arcadia/oa#`   | Operational Analysis (Actors, Activities)      |
| **Système**      | `sa:`   | `.../arcadia/sa#`   | System Analysis (System Functions, Components) |
| **Logique**      | `la:`   | `.../arcadia/la#`   | Logical Architecture                           |
| **Physique**     | `pa:`   | `.../arcadia/pa#`   | Physical Architecture (Nodes, Boards)          |
| **EPBS**         | `epbs:` | `.../arcadia/epbs#` | End Product Breakdown Structure (CIs)          |
| **Données**      | `data:` | `.../arcadia/data#` | Data Modeling (Classes, Exchange Items)        |

## 🚀 Fonctionnalités Clés

### 1\. Expansion et Compaction

C'est le cœur du JSON-LD. Cela permet de normaliser les données avant traitement.

- **Expansion** : Transforme les clés courtes en IRIs complets. Utile pour vérifier les types de manière absolue.
  - Entrée : `{"@type": "oa:OperationalActivity"}`
  - Sortie : `{"@type": "https://raise.io/ontology/arcadia/oa#OperationalActivity"}`
- **Compaction** : L'inverse. Transforme les IRIs complets en clés courtes pour le stockage ou l'affichage, en utilisant le contexte actif.

### 2\. Validation Sémantique

Contrairement à la validation de schéma (structurelle), la validation sémantique vérifie le sens des données.

- **`validate_required_fields`** : Vérifie la présence de champs en utilisant leur identité sémantique (IRI), peu importe le préfixe utilisé dans le JSON.
- **Vérification de Vocabulaire** : Le `CollectionsManager` utilise ce module pour vérifier si un `@type` déclaré dans un document existe réellement dans le `VocabularyRegistry`, émettant un avertissement si le type est inconnu.

### 3\. Export RDF

Le module peut convertir un document JSON-LD en triplets RDF (format N-Triples), ce qui permet l'interopérabilité avec d'autres outils du Web Sémantique (Protégé, GraphDB, etc.).

## 🛠️ Utilisation

```rust
use crate::json_db::jsonld::{JsonLdProcessor, VocabularyRegistry};
use serde_json::json;

// 1. Instanciation
let processor = JsonLdProcessor::new();

// 2. Document JSON avec contexte
let doc = json!({
    "@context": { "oa": "https://raise.io/ontology/arcadia/oa#" },
    "@type": "oa:OperationalActivity",
    "oa:name": "Analyser le besoin"
});

// 3. Expansion (Accès aux données normalisées)
let expanded = processor.expand(&doc);
// expanded["@type"] vaut maintenant l'URI complète

// 4. Validation Vocabulaire
let registry = VocabularyRegistry::new();
let type_iri = processor.get_type(&doc).unwrap();
let expanded_type = processor.context_manager().expand_term(&type_iri);

if registry.has_class(&expanded_type) {
    println!("Classe valide : {}", expanded_type);
}
```

## 📂 Structure des Fichiers

```text
src-tauri/src/json_db/jsonld/
├── mod.rs          // Point d'entrée, exports et structures de sérialisation
├── context.rs      // Gestion des préfixes et contextes (@context)
├── processor.rs    // Algorithmes JSON-LD (Expand, Compact, RDF)
├── vocabulary.rs   // Définitions statiques de l'ontologie Arcadia
└── tests.rs        // Tests unitaires
```

## ⚠️ Notes Techniques

- **Registre en Mémoire** : Le `VocabularyRegistry` est actuellement défini en dur dans le code Rust (`vocabulary.rs`). Il ne charge pas d'ontologies externes (`.owl` ou `.ttl`) dynamiquement au runtime.
- **Validation Légère** : Ce module n'est pas un validateur SHACL ou OWL complet. Il effectue des vérifications d'existence de termes et de champs requis basiques.
- **Standards** : Le module suit les concepts de JSON-LD 1.1 mais n'implémente pas la totalité de la spécification W3C (ex: pas de chargement de contextes distants via HTTP pour des raisons de performance et de sécurité locale).
