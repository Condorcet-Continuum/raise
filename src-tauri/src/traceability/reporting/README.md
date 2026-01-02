# Module de Reporting

Ce module est responsable de la **génération d'artefacts** basés sur l'analyse du modèle. Il transforme les données brutes du graphe (liens) et les résultats de conformité en structures de données exploitables pour l'interface utilisateur (tableaux de bord) ou pour l'exportation (fichiers d'audit).

## 📊 Fonctionnalités

Le module se divise en deux générateurs principaux :

### 1. Matrice de Traçabilité

_Fichier : `trace_matrix.rs`_

Génère des vues croisées entre deux couches d'architecture (ex: Analyse Système vs Architecture Logique) pour visualiser la couverture.

- **Calcul de Couverture** : Détermine automatiquement le statut (`Covered`, `Uncovered`) de chaque élément source.
- **Support** :
  - **SA ➔ LA** : Vérifie comment les Fonctions Système (SA) sont réalisées par les Composants Logiques (LA).
  - _(Extensible pour d'autres transitions : OA ➔ SA, LA ➔ PA)_.

**Format de sortie (JSON) :**

```json
{
  "rows": [
    {
      "source_id": "func_sa_01",
      "source_name": "Calculer Trajectoire",
      "target_ids": ["comp_la_nav_01", "comp_la_nav_02"],
      "coverage_status": "Covered"
    },
    {
      "source_id": "func_sa_02",
      "source_name": "Afficher Alerte",
      "target_ids": [],
      "coverage_status": "Uncovered"
    }
  ]
}
```

### 2\. Rapport d'Audit Global

_Fichier : `audit_report.rs`_

Orchestre la génération d'un rapport complet sur la santé du projet. Il agit comme point d'entrée unique pour :

1.  Calculer les **statistiques volumétriques** du modèle (nombre de fonctions, composants, etc.).
2.  Exécuter tous les **Checkers de Conformité** (via le module `../compliance`), incluant désormais les règles **EU AI Act**.
3.  Aggréger les résultats dans un objet structuré.

**Format de sortie (JSON) :**

```json
{
  "project_name": "RAISE Project",
  "date": "2025-12-10T14:30:00Z",
  "model_stats": {
    "total_elements": 150,
    "total_functions": 45,
    "total_components": 30
  },
  "compliance_results": [
    {
      "standard": "DO-178C (Software Considerations in Airborne Systems)",
      "passed": false,
      "violations": [{ "rule_id": "DO178-HLR-01", "severity": "High", "description": "..." }]
    },
    {
      "standard": "EU AI Act (Transparency & Record-keeping)",
      "passed": true,
      "violations": []
    }
  ]
}
```

---

## 💻 Utilisation (Rust)

Les générateurs sont des méthodes statiques sans état interne (stateless), prenant une référence au `ProjectModel`.

```rust
use crate::traceability::reporting::{audit_report::AuditGenerator, trace_matrix::MatrixGenerator};

// Générer la matrice SA -> LA
let matrix = MatrixGenerator::generate_sa_to_la(&project_model);

// Générer le rapport d'audit complet (incluant DO-178C, ISO-26262, EU AI Act)
let audit = AuditGenerator::generate(&project_model);
```

## 🚀 Extension

Pour ajouter un nouveau type de rapport (ex: Export CSV plat des exigences) :

1.  Créer un nouveau fichier (ex: `src-tauri/src/traceability/reporting/csv_export.rs`).
2.  Implémenter une structure capable de parcourir le `ProjectModel` via le `Tracer`.
3.  Exposer le module dans `mod.rs`.

<!-- end list -->

```


```
