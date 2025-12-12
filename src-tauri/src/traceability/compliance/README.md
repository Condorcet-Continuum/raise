# Module de Conformité (Compliance)

Ce module est responsable de la **vérification statique** des modèles d'architecture système (Arcadia). Il analyse le graphe des éléments (Fonctions, Composants, Exigences) pour s'assurer qu'ils respectent les règles définies par des standards industriels critiques (Avionique, Automobile, IA).

## 📋 Standards Supportés

Le moteur est conçu pour être extensible. Actuellement, les vérificateurs (Checkers) suivants sont implémentés :

### 1. DO-178C (Avionique)

_Fichier : `do_178c.rs`_
Se concentre sur la traçabilité des exigences logicielles.

- **Règle HLR-01 :** Tout composant de l'Architecture Physique (PA - Software Component) doit avoir un lien de traçabilité explicite (allocation ou réalisation) vers une fonction ou un composant logique (Exigences de haut niveau).

### 2. ISO-26262 (Automobile)

_Fichier : `iso_26262.rs`_
Gère la sécurité fonctionnelle et les niveaux d'intégrité (ASIL).

- **Règle ASIL-D :** Si une fonction est marquée avec un niveau `ASIL=D`, elle doit obligatoirement définir une propriété `safetyMechanism` pour mitiger les risques.

### 3. IEC-61508 (Industriel)

_Fichier : `iec_61508.rs`_
Structure de base pour la sécurité fonctionnelle des systèmes électroniques/programmables (en cours d'implémentation).

### 4. EU AI Act (Régulation IA)

_Fichier : `eu_ai_act.rs`_
Assure la transparence et la traçabilité technique des systèmes d'Intelligence Artificielle.

- **Règle AI-ACT-TRANS-01 :** Tout composant identifié comme modèle d'IA (`type="AI_Model"`) doit posséder une référence valide vers une preuve d'explicabilité (**XAI Frame**) pour garantir qu'il n'est pas une "boîte noire" totale.

---

## Architecture Technique

Le système repose sur le trait `ComplianceChecker`. Chaque standard est une structure qui implémente ce trait.

```rust
pub trait ComplianceChecker {
    /// Nom lisible du standard
    fn name(&self) -> &str;

    /// Exécute l'analyse sur le modèle complet et retourne un rapport
    fn check(&self, model: &ProjectModel) -> ComplianceReport;
}
```

### Structures de Données

- **ComplianceReport** : Résultat global contenant le statut (Pass/Fail) et la liste des violations.
- **Violation** : Détail d'une erreur incluant l'ID de l'élément fautif, l'ID de la règle enfreinte, une description et la sévérité.

---

## 🛠 Comment ajouter un nouveau standard

Pour ajouter un nouveau standard (par exemple, _ECSS_ pour le spatial) :

1.  **Créer le fichier** : Ajoutez `src-tauri/src/traceability/compliance/ecss.rs`.
2.  **Implémenter le Trait** :

    ```rust
    use super::{ComplianceChecker, ComplianceReport, Violation};
    use crate::model_engine::types::ProjectModel;

    pub struct EcssChecker;

    impl ComplianceChecker for EcssChecker {
        fn name(&self) -> &str { "ECSS-E-ST-40C" }
        fn check(&self, model: &ProjectModel) -> ComplianceReport {
            // Logique de vérification ici...
        }
    }
    ```

3.  **Enregistrer le module** : Ajoutez `pub mod ecss;` dans `mod.rs`.
4.  **Intégrer au Rapport** : Ajoutez le checker dans la liste `checkers` du fichier `../reporting/audit_report.rs`.

---

## Tests

Les tests unitaires sont situés directement dans les fichiers sources (`#[cfg(test)]`). Pour lancer les tests de conformité uniquement :

```bash
cargo test traceability::compliance
```
