// FICHIER : src-tauri/src/workflow_engine/critic.rs

use crate::ai::assurance::xai::XaiFrame;
use serde::{Deserialize, Serialize};

/// Résultat de l'évaluation d'une action par le critique
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CritiqueResult {
    pub score: f32,          // Score de 0.0 à 1.0
    pub is_acceptable: bool, // Seuil de validation
    pub reasoning: String,   // Pourquoi ce score ?
}

/// Le Critique évalue la qualité des sorties du système (Reward Model)
pub struct WorkflowCritic {
    // Seuil minimal pour considérer une action comme réussie
    threshold: f32,
}

impl Default for WorkflowCritic {
    fn default() -> Self {
        Self { threshold: 0.7 }
    }
}

impl WorkflowCritic {
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }

    /// Analyse une XaiFrame pour déterminer la qualité de la réponse
    pub async fn evaluate(&self, frame: &XaiFrame) -> CritiqueResult {
        // 1. Critères Heuristiques (Rapides)
        let output_len = frame.predicted_output.len();

        // Pénalité si la réponse est vide
        if output_len == 0 {
            return CritiqueResult {
                score: 0.0,
                is_acceptable: false,
                reasoning: "La sortie est vide.".into(),
            };
        }

        // 2. Critères de Contenu (Exemple basique)
        // CORRECTION ICI : Typage explicite en f32 pour éviter l'ambiguïté
        let mut score: f32 = 0.8;
        let mut notes = Vec::new();

        // Exemple de règle : Si l'input demandait du JSON, on vérifie si ça ressemble à du JSON
        if frame.input_snapshot.to_lowercase().contains("json") {
            if frame.predicted_output.trim().starts_with('{')
                || frame.predicted_output.trim().starts_with('[')
            {
                score += 0.1;
                notes.push("Format JSON détecté (+0.1)");
            } else {
                score -= 0.3;
                notes.push("Format JSON attendu mais non détecté (-0.3)");
            }
        }

        // Normalisation (Clamp) entre 0.0 et 1.0
        score = score.clamp(0.0, 1.0);

        CritiqueResult {
            score,
            is_acceptable: score >= self.threshold,
            reasoning: if notes.is_empty() {
                "Évaluation standard.".into()
            } else {
                notes.join(", ")
            },
        }
    }
}
