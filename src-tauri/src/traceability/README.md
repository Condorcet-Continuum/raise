# Module de Traçabilité (Traceability Engine)

Ce module constitue le cœur de l'analyse d'impact et de la vérification système de **RAISE**. Il est responsable de l'interprétation des relations entre les éléments du modèle Arcadia (Operational, System, Logical, Physical) pour garantir la cohérence et la conformité du projet.

## 🎯 Objectifs

1.  **Navigation Bidirectionnelle** : Permettre de parcourir le graphe des éléments aussi bien en aval (Allocations/Réalisations) qu'en amont (Liens inverses).
2.  **Analyse d'Impact** : Identifier les conséquences d'une modification sur le reste du système.
3.  **Vérification de Conformité** : Assurer que le modèle respecte les normes critiques :
    - **Avionique** (DO-178C)
    - **Automobile** (ISO-26262)
    - **Régulation IA** (EU AI Act - Transparence & Robustesse)
4.  **Reporting** : Générer des matrices de preuves et des rapports d'audit unifiés.

## 📂 Structure du Module

| Fichier / Dossier        | Responsabilité                                                                                                   |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------- |
| **`mod.rs`**             | Point d'entrée, expose les sous-modules publics.                                                                 |
| **`tracer.rs`**          | **Moteur principal.** Indexe les liens et fournit les méthodes de navigation (`get_upstream`, `get_downstream`). |
| **`impact_analyzer.rs`** | Algorithme de propagation. Calcule la portée et la criticité d'un changement potentiel.                          |
| **`change_tracker.rs`**  | Utilitaire de comparaison (Diff) entre deux versions JSON d'un même élément.                                     |
| **`compliance/`**        | [Sous-module](./compliance/README.md) contenant les règles de validation (incluant désormais **EU AI Act**).     |
| **`reporting/`**         | [Sous-module](./reporting/README.md) générant les artefacts de sortie (Matrices, Audits).                        |

## 🚀 Utilisation

Voici comment les différents composants interagissent typiquement au sein de l'application (ex: depuis une commande Tauri) :

### 1. Navigation simple (Tracer)

Récupérer ce qui est impacté par une Fonction Système.

```rust
use crate::traceability::tracer::Tracer;

let tracer = Tracer::new(&project_model);

// "Qui réalise cette fonction ?" (Vers le bas / Downstream)
let components = tracer.get_downstream_elements("uuid_fonction_sa");

// "Qui demande cette fonction ?" (Vers le haut / Upstream)
let requirements = tracer.get_upstream_elements("uuid_fonction_sa");
```

### 2\. Analyse d'Impact

Calculer le score de criticité avant une modification.

```rust
use crate::traceability::impact_analyzer::ImpactAnalyzer;

let tracer = Tracer::new(&project_model);
let analyzer = ImpactAnalyzer::new(tracer);

// Analyse jusqu'à 5 niveaux de profondeur
let report = analyzer.analyze("uuid_element_modifie", 5);

println!("Score de criticité : {}", report.criticality_score);
println!("Éléments touchés : {:?}", report.impacted_elements);
```

### 3\. Audit Complet

Générer un rapport de santé du projet incluant les preuves d'assurance IA.

```rust
use crate::traceability::reporting::audit_report::AuditGenerator;

let audit = AuditGenerator::generate(&project_model);

// Sérialisation pour le frontend (JSON contenant DO-178C, EU AI Act, etc.)
let json_output = serde_json::to_string(&audit).unwrap();
```

## 🧠 Concepts Clés

- **Upstream (Amont)** : Désigne les éléments "parents" ou demandeurs (ex: Une Exigence est en amont d'une Fonction). Le `Tracer` reconstruit ces liens dynamiquement via un index inversé.
- **Downstream (Aval)** : Désigne les éléments "enfants" ou réalisateurs (ex: Un Composant est en aval d'une Fonction).
- **Couverture** : Un élément est dit "couvert" s'il possède au moins un lien vers l'aval.
- **Preuve IA** : Le moteur vérifie l'existence de liens vers des trames XAI (générées par `src/ai/assurance`) pour valider la conformité des composants marqués comme "AI_Model".

## ✅ Tests

L'ensemble de la logique de traçabilité est couverte par des tests unitaires intégrés.

```bash
# Lancer tous les tests de traçabilité (moteur, compliance, reporting)
cargo test traceability
```

```

```
