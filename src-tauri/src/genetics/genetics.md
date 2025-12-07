Ce document structure la vision, l'architecture technique et l'intégration des algorithmes génétiques dans l'écosystème GenAptitude, en mettant l'accent sur l'approche **Neuro-Symbolique** et la **Traçabilité**.

---

# Module `genetics` — Moteur d'Optimisation Évolutif

**Package :** `genetics`
**Dépendances clés :** `rayon` (parallélisme), `rand`, `serde`
**Rôle :** Moteur d'optimisation globale pour l'exploration d'architectures et la neuro-évolution.

---

## 1\. Vision Neuro-Symbolique

Le module `genetics` n'est pas une simple bibliothèque d'algorithmes. C'est le **"Designer Automatisé"** de GenAptitude. Il comble le fossé entre l'IA générative (LLM) et l'ingénierie formelle (Arcadia) :

1.  **Exploration Structurelle (Symbolique)** : Il manipule des structures de graphes explicites (Architectures Arcadia, Arbres de décision) pour trouver des solutions valides respectant des contraintes strictes.
2.  **Optimisation Globale (Numérique)** : Il permet d'optimiser des hyper-paramètres ou des topologies de réseaux de neurones (Neuro-Évolution) là où la descente de gradient est impossible.
3.  **Auditabilité (GenAptitude Core)** : Chaque étape de l'évolution est sérialisable. On ne garde pas juste le résultat, mais la généalogie de la solution, stockée dans la `json_db`.

---

## 2\. Architecture du Module

L'architecture est découplée pour séparer le moteur d'exécution (générique) des problèmes métiers (spécifiques).

```text
src-tauri/src/genetics/
├── mod.rs                     # Exports publics
├── engine.rs                  # Boucle d'évolution (High Performance / Rayon)
├── traits.rs                  # Contrats (Genome, Evaluator, Operator)
├── types.rs                   # Structures de données (Population, Stats)
├── commands.rs                # API Tauri (start_optimization, get_stats)
├── operators/                 # Bibliothèque d'opérateurs génétiques
│   ├── selection.rs           # (ex: Tournament, Roulette)
│   ├── crossover.rs           # (ex: Uniform, OnePoint)
│   └── mutation.rs            # (ex: Gaussian, BitFlip)
├── genomes/                   # Représentations des solutions
│   ├── arcadia_arch.rs        # Allocation de fonctions système
│   ├── neural_net.rs          # Topologie de réseaux (Neuro-evolution)
│   └── decision_tree.rs       # Règles logiques (Explainable AI)
└── evaluators/                # Fonctions de Fitness (Lien métier)
    └── constraints.rs         # Vérification via Model Engine
```

---

## 3\. Concepts Clés (`traits.rs`)

### Le Trait `Genome`

Définit une solution candidate. Elle doit être clonable, sérialisable (pour la DB) et thread-safe (pour `rayon`).

```rust
pub trait Genome: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> {
    fn random() -> Self;
    fn mutate(&mut self, rate: f32);
    fn crossover(&self, other: &Self) -> Self;
}
```

### Le Trait `Evaluator`

Définit la qualité d'une solution. C'est ici que l'on connecte le **Model Engine** pour vérifier les règles métiers (Arcadia).

```rust
pub trait Evaluator<G: Genome>: Send + Sync {
    /// Retourne un score (plus haut = meilleur).
    fn evaluate(&self, genome: &G) -> f32;

    /// Vérifie les contraintes dures (ex: "Une fonction doit avoir un port").
    fn is_valid(&self, genome: &G) -> bool { true }
}
```

---

## 4\. Flux d'Exécution & Performance

Le moteur utilise **Rayon** pour paralléliser l'évaluation, qui est souvent l'étape la plus coûteuse en CPU.

```mermaid
graph TD
    Start[Commande Tauri: run_optimization] --> Init[Population Initiale (Aléatoire)]

    subgraph "Boucle Évolutive (Engine)"
        Eval[Évaluation Parallèle (Rayon)]
        Select[Sélection (Tournoi)]
        Cross[Croisement (Reproduction)]
        Mut[Mutation (Diversité)]

        Init --> Eval
        Eval -->|Fitness Scores| Check{Critère d'arrêt ?}
        Check -- Non --> Select
        Select --> Cross
        Cross --> Mut
        Mut --> Eval
    end

    Check -- Oui --> Persist[Persistance JSON-DB]
    Persist --> Report[Rapport d'Optimisation]
```

---

## 5\. Cas d'Usage : Optimisation d'Architecture (SA)

Un cas concret pour GenAptitude est l'**allocation optimale de fonctions sur des composants** pour minimiser la latence tout en respectant un budget énergétique.

### Le Génome (`genomes/arcadia_arch.rs`)

```rust
pub struct SystemAllocationGenome {
    // Map: ID Fonction (SA) -> ID Composant (SA)
    pub allocations: HashMap<String, String>,
}
```

### L'Évaluateur (`evaluators/system_cost.rs`)

1.  **Contrainte (Validité)** : Chaque fonction doit être allouée à un composant qui a les interfaces requises.
2.  **Fitness (Score)** :
    - `+100` si la latence totale \< 50ms.
    - `-10 * Coût` (pénalité sur le coût matériel).
    - `-50` si un composant est surchargé (CPU \> 90%).

---

## 6\. Intégration avec la Persistance (`json_db`)

Pour garantir la traçabilité (Audit Trail), chaque run d'optimisation est enregistré.

**Collection :** `optimizations`
**Schéma :**

```json
{
  "id": "run-uuid-1234",
  "type": "optimization_run",
  "algorithm": "genetic_v1",
  "parameters": {
    "population_size": 100,
    "generations": 50,
    "mutation_rate": 0.05
  },
  "best_solution": {
    "fitness": 98.5,
    "genome": { ... }, // L'architecture gagnante sérialisée
    "generation_found": 42
  },
  "createdAt": "2025-11-30T10:00:00Z"
}
```

---

## 7\. Roadmap Technique

- **v0.1.0** : Moteur générique simple + Opérateurs standards + Exemple `BitString`.
- **v0.2.0** : Intégration `genomes/arcadia` + Persistance `json_db`.
- **v0.3.0** : **Interactive Evolution** (L'utilisateur humain note les solutions proposées par l'AG dans l'UI Tauri).
- **v1.0.0** : **Neuro-Évolution** (L'AG optimise l'architecture d'un réseau de neurones local pour les agents IA).

## 8\. Intégration Sémantique & Graphe de Connaissance (JSON-LD)

L'un des différentiateurs majeurs de GenAptitude est que l'optimisation n'est pas une "boîte noire". Chaque étape de l'évolution enrichit le graphe de connaissance du projet.

Nous utilisons l'ontologie **PROV-O** (Provenance Ontology) standard couplée à un vocabulaire dédié `genetics`.

### 8.1. Vocabulaire et Ontologie

Le module `genetics` introduit un nouveau namespace dans le `SchemaRegistry` :

- **Prefix** : `gen`
- **URI** : `https://genaptitude.io/ontology/genetics#`

| Concept              | Type JSON-LD          | Description                                                     |
| :------------------- | :-------------------- | :-------------------------------------------------------------- |
| **Optimization Run** | `gen:OptimizationRun` | Une exécution complète d'un AG. Agit comme une `prov:Activity`. |
| **Solution**         | `gen:Solution`        | Un individu spécifique (Génome + Fitness).                      |
| **Lineage**          | `prov:wasDerivedFrom` | Lien de parenté (Parent A + Parent B -\> Enfant).               |
| **Target**           | `gen:optimizes`       | Lien vers l'élément Arcadia ciblé (ex: `sa:SystemComponent`).   |

### 8.2. Exemple de Document Sémantique

Lorsqu'un run d'optimisation est sauvegardé dans `json_db/collections/optimizations/`, il ressemble à ceci :

```json
{
  "@context": [
    "https://genaptitude.io/ontology/arcadia/core.jsonld",
    {
      "gen": "https://genaptitude.io/ontology/genetics#",
      "prov": "http://www.w3.org/ns/prov#",
      "fitness": "gen:fitnessScore",
      "parameters": "gen:hyperParameters"
    }
  ],
  "id": "urn:uuid:run-opt-2025-a7x9",
  "@type": ["gen:OptimizationRun", "prov:Activity"],
  "name": "Optimisation Latence/Coût - Serveur Vidéo",

  // Ancrage : Quel élément du modèle est optimisé ?
  "gen:optimizes": {
    "@id": "urn:uuid:sa-component-srv-video",
    "@type": "sa:SystemComponent"
  },

  // Configuration de l'algorithme
  "parameters": {
    "mutation_rate": 0.05,
    "population_size": 200,
    "generations": 50
  },

  // Le Champion (Meilleure solution trouvée)
  "gen:bestSolution": {
    "@type": "gen:Solution",
    "fitness": 98.4,

    // Le Génome est ici une représentation partielle d'un élément Arcadia
    "gen:genome": {
      "@type": "sa:SystemComponent",
      "name": "Serveur Vidéo (Optimisé v42)",
      "propertyValues": {
        "cpu_cores": 8,
        "ram_gb": 32
      }
    },

    // Traçabilité : D'où vient cette solution ?
    "prov:wasDerivedFrom": [
      { "@id": "urn:uuid:solution-gen41-id88" }, // Parent A
      { "@id": "urn:uuid:solution-gen41-id12" } // Parent B
    ]
  },

  "createdAt": "2025-11-30T14:00:00Z"
}
```

### 8.3. Apport pour l'IA Neuro-Symbolique

Grâce à cette structure JSON-LD, les **Agents IA** (`SystemAgent`) peuvent requêter le graphe pour "comprendre" l'évolution :

1.  **Explicabilité** : _"Pourquoi cette architecture a-t-elle été choisie ?"_
    - _Réponse via Graphe :_ "Elle provient du Run `run-opt-2025` qui a maximisé le score de fitness (98.4) en privilégiant le coût sur la latence."
2.  **Réutilisation** : Un agent peut récupérer les meilleurs génomes d'anciens runs pour **seeding** (initialiser) une nouvelle population, accélérant ainsi la convergence (Transfer Learning symbolique).

### 8.4. Visualisation du Graphe

```mermaid
graph TD
    User((Utilisateur)) -->|Lance| Run[Optimization Run]
    Run -->|Cible| Component[System Component (SA)]
    Run -->|Produit| BestSol[Best Solution]

    BestSol -->|Contient| Genome[Optimized Genome]
    BestSol -.->|Dérivé de| ParentA[Solution Gen 49]
    BestSol -.->|Dérivé de| ParentB[Solution Gen 49]

    Genome --"Est une version de"--> Component
```
