# Module `ai/agents` — Système Multi-Agents Neuro-Symbolique

Ce module implémente la logique **exécutive** de l'IA de RAISE. Il transforme des requêtes en langage naturel (floues) en artefacts d'ingénierie formels (strictes, validés et persistés) selon la méthodologie **Arcadia**.

## 🧠 Architecture Globale

Le système repose sur un pipeline **Comprendre → Décider → Agir** orchestré par un Dispatcher central.

```text
┌──────────────┐
│  UTILISATEUR │
└──────┬───────┘
       │ "Crée une exigence de performance"
       ▼
┌──────────────────────┐         1. Classification         ┌───────────────────┐
│      DISPATCHER      │ ────────────────────────────────▶ │ INTENT CLASSIFIER │
│   (ai_commands.rs)   │ ◀──────────────────────────────── │ (Mode JSON Strict)│
└──────────┬───────────┘         2. EngineeringIntent      └─────────┬─────────┘
           │                                                         │
           │ 3. Routage (Layer = TRANSVERSE)                         │
           ▼                                                         ▼
┌──────────────────────┐                                   ┌───────────────────┐
│     AGENT SQUAD      │         4. Génération             │        LLM        │
│  (TransverseAgent)   │ ────────────────────────────────▶ │  (Local / Cloud)  │
└──────────┬───────────┘ ◀──────────────────────────────── └───────────────────┘
           │                     5. JSON Détaillé (Brut)
           │
           │ 6. Écriture (Validation Schéma + UUID)
           ▼
┌──────────────────────┐
│       JSON-DB        │
│   (StorageEngine)    │
└──────────────────────┘
           │
           │ 7. AgentResult { message, artifacts: [...] }
           ▼
    VERS FRONTEND

```

---

## 👥 La "Squad" d'Agents (Spécialisation)

Contrairement à une approche monolithique, RAISE utilise une **équipe d'agents spécialisés**, chacun expert dans sa couche d'abstraction Arcadia.

| Agent               | Rôle & Responsabilités | Couche         | Schémas gérés                                              |
| ------------------- | ---------------------- | -------------- | ---------------------------------------------------------- |
| **BusinessAgent**   | Analyste Métier        | **OA**         | `OperationalCapability`, `OperationalActor`                |
| **SystemAgent**     | Architecte Système     | **SA**         | `SystemFunction`, `SystemComponent`, `SystemActor`         |
| **SoftwareAgent**   | Architecte Logiciel    | **LA**         | `LogicalComponent` + **Génération de Code**                |
| **HardwareAgent**   | Architecte Matériel    | **PA**         | `PhysicalNode` (Détection auto: Électronique vs Infra)     |
| **EpbsAgent**       | Config Manager         | **EPBS**       | `ConfigurationItem` (Gestion P/N, Kind)                    |
| **DataAgent**       | Data Architect         | **DATA**       | `Class`, `DataType`, `ExchangeItem` (MDM)                  |
| **TransverseAgent** | Qualité & IVVQ Manager | **TRANSVERSE** | `Requirement`, `Scenario`, `TestProcedure`, `TestCampaign` |

---

## 🛡️ Robustesse & Tolérance aux Pannes

Le module a été durci pour fonctionner avec des **Small Language Models (SLM)** locaux (ex: Mistral, Llama 3) qui sont souvent "bavards" ou imprécis.

### 1. Parsing "Chirurgical" (`extract_json`)

Les agents n'essaient plus de parser toute la réponse du LLM. Ils utilisent une méthode d'extraction intelligente :

- Ignorer les balises Markdown (````json`).
- Repérer la première accolade `{` et la dernière `}`.
- Couper tout le texte explicatif avant ou après.

### 2. Intent Classifier Tolérant

- **Structure Plate** : `{ "intent": "...", "layer": "SA" }` (plus robuste que les structures imbriquées).
- **Champs Optionnels** : Utilisation de `#[serde(default)]` pour les champs comme `context` dans la génération de code, évitant les crashs si le LLM oublie un paramètre mineur.

### 3. Protection "Force Name"

Pour éviter que l'IA ne renomme arbitrairement les éléments (ex: "Rack Server" -> "Server"), les agents écrasent systématiquement le champ `name` du JSON généré avec la demande initiale de l'utilisateur.

---

## 📦 Sortie Structurée : `AgentResult`

Pour permettre une UI riche, les agents ne renvoient pas une simple chaîne de caractères, mais une structure `AgentResult` :

```rust
pub struct AgentResult {
    pub message: String,              // Feedback textuel (Markdown)
    pub artifacts: Vec<CreatedArtifact>, // Liste des objets créés
}

pub struct CreatedArtifact {
    pub id: String,
    pub name: String,
    pub layer: String,        // Ex: "SA"
    pub element_type: String, // Ex: "Function"
    pub path: String,         // Chemin relatif pour ouverture dans l'UI
}

```

Cela permet au Frontend d'afficher des **"Cartes d'Artefacts"** cliquables dans le chat.

---

## 🚀 Utilisation & Tests

### Via la Suite de Tests (Recommandé)

Le projet dispose d'une suite de tests d'intégration complète validant le cycle en V.

```bash
# Lancer toute la suite IA (Agents + Code Gen)
cargo test --test ai_suite -- --ignored
cargo test --test code_gen_suite -- --ignored

# Tester un agent spécifique (ex: Data)
cargo test --test ai_suite data_agent_tests -- --ignored --nocapture

```

### Via le CLI

```bash
# Exemple : Création d'une procédure de test
cargo run -p ai_cli -- classify "Crée un test pour vérifier le login" -x

```

## 🔮 Roadmap Technique

- [ ] **Gestion des Relations (WIP)** : Implémentation complète des `DataFlow` et `ComponentExchange` (actuellement en migration).
- [ ] **Mode RAG Avancé** : Indexation vectorielle des Exigences pour la vérification de cohérence.
- [ ] **Review Agent** : Un agent dédié à l'audit des modèles (Quality Rules).

<!-- end list -->

```

```
