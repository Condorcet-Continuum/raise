# Module d'Assurance IA (AI Assurance)

Ce module fournit les structures de données standardisées pour capturer, stocker et transporter les **preuves de confiance** des modèles d'Intelligence Artificielle de GenAptitude.

Il ne réalise pas l'inférence (gérée par `../inference.rs`), mais il est responsable de la **documentation technique** nécessaire à la conformité réglementaire (notamment l'EU AI Act).

## 🎯 Objectifs

1.  **Explicabilité (XAI)** : Standardiser le format des explications (SHAP, Attention Maps, LIME) pour qu'elles soient lisibles par le Frontend et vérifiables par le moteur de traçabilité.
2.  **Qualité & Robustesse** : Structurer les rapports de tests (Performance, Biais, Équité) pour valider qu'un modèle est apte à la production.
3.  **Interopérabilité** : Servir de langage commun entre l'exécution (Python/Rust/ONNX) et la vérification (Traceability Engine).

## 📂 Structure du Module

| Fichier          | Description                                                                                                                           |
| :--------------- | :------------------------------------------------------------------------------------------------------------------------------------ |
| **`mod.rs`**     | Point d'entrée, expose les types publics (`XaiFrame`, `QualityReport`).                                                               |
| **`xai.rs`**     | Définit la trame d'explicabilité (**XaiFrame**). Supporte les données tabulaires (Feature Importance) et visuelles (Heatmaps).        |
| **`quality.rs`** | Définit le rapport de validation (**QualityReport**). Gère les seuils de succès/échec pour la Performance, la Robustesse et l'Équité. |

---

## 🔍 1. Explicabilité (`xai.rs`)

La structure centrale est `XaiFrame`. Elle capture "Pourquoi le modèle a pris cette décision".

### Fonctionnalités Clés

- **Multi-méthodes** : Supporte SHAP, LIME, Attention Maps, Integrated Gradients, etc.
- **Multi-supports** : Peut stocker des listes pondérées (pour les données tabulaires) et des **Visual Artifacts** (images Base64, SVG) pour l'affichage UI.
- **Scope** : Distingue les explications **Locales** (une inférence précise) des explications **Globales** (comportement général du modèle).

### Exemple d'utilisation

```rust
use crate::ai::assurance::xai::{XaiFrame, XaiMethod, ExplanationScope};

// Création d'une trame après une inférence
let mut frame = XaiFrame::new(
    "model_credit_v1",
    XaiMethod::Shap { variant: "TreeShap".into() },
    ExplanationScope::Local
);

// Ajout de contexte
frame.input_snapshot = "Revenu: 30k, Dette: Haute".to_string();
frame.predicted_output = "Refus".to_string();

// Ajout des facteurs explicatifs
frame.add_feature("Dette_Totale", "50000", -0.45, 1);
frame.add_feature("Revenu", "30000", 0.15, 2);

// Ajout d'un visuel (ex: pour le frontend)
frame.add_visual("heatmap", "image/png", "base64_string...");
```

## 🛡️ 2. Qualité (`quality.rs`)

La structure centrale est `QualityReport`. Elle agit comme un "certificat de contrôle technique" du modèle.

### Catégories de Métriques

- **Performance** : Accuracy, F1-Score, RMSE.
- **Robustness** : Stabilité face au bruit, taux de succès contre attaques adverses.
- **Fairness** : Parité statistique, égalité des chances (biais démographiques).
- **Efficiency** : Latence, consommation mémoire.

### Logique de Validation

Le rapport calcule automatiquement un statut global (`Pass`, `Warning`, `Fail`) basé sur la criticité des métriques échouées.

### Exemple d'utilisation

```rust
use crate::ai::assurance::quality::{QualityReport, MetricCategory};

let mut report = QualityReport::new("model_credit_v1", "dataset_test_2025");

// Ajout d'une métrique critique (Doit être > 0.90)
report.add_metric(
    "Accuracy",
    MetricCategory::Performance,
    0.95,       // Valeur mesurée
    Some(0.90), // Seuil Min
    None,       // Seuil Max
    true        // Critique (Fail si échoué)
);

// Ajout d'une métrique informative (Latence < 50ms)
report.add_metric(
    "Latency",
    MetricCategory::Efficiency,
    45.0,
    None,
    Some(50.0),
    false
);
```

---

## 🔗 Intégration avec la Traçabilité

Ce module fonctionne en tandem avec `src-tauri/src/traceability`.

1.  **Génération** : Le module `ai` génère ces objets (`XaiFrame`, `QualityReport`).
2.  **Liaison** : Les IDs de ces objets sont stockés dans les propriétés des composants du modèle d'architecture (Physical Architecture).
3.  **Vérification** : Le module `traceability/compliance/eu_ai_act.rs` scanne le modèle pour vérifier que chaque composant IA possède bien ces preuves associées.

> **Note :** Ce découpage assure que le moteur de traçabilité reste léger et ne dépend pas des lourdes bibliothèques de calcul d'IA.

```

```
