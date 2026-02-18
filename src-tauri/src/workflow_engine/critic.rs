// FICHIER : src-tauri/src/workflow_engine/critic.rs
use crate::utils::prelude::*;

use crate::ai::assurance::xai::XaiFrame;

/// Résultat de l'évaluation d'une action par le critique
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CritiqueResult {
    pub score: f32,          // Score de 0.0 à 1.0
    pub is_acceptable: bool, // Verdict binaire (Passe / Passe pas)
    pub reasoning: String,   // Explication pour l'audit
}

/// Le Critique évalue la qualité des sorties du système (Reward Model simplifié)
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

    /// Analyse une XaiFrame (snapshot d'exécution IA) pour déterminer la qualité de la réponse.
    pub async fn evaluate(&self, frame: &XaiFrame) -> CritiqueResult {
        // 1. Critères Heuristiques (Vérifications rapides)
        let output_len = frame.predicted_output.len();

        // Règle 0 : Une réponse vide est inacceptable
        if output_len == 0 {
            return CritiqueResult {
                score: 0.0,
                is_acceptable: false,
                reasoning: "La sortie est vide.".into(),
            };
        }

        // 2. Critères de Contenu (Logique métier simulée)
        // Note: Dans le futur, ceci pourrait appeler un LLM "Juge" (LLM-as-a-Judge)

        let mut score: f32 = 0.8; // Score de base optimiste
        let mut notes = Vec::new();

        // Règle 1 : Vérification de format JSON si demandé
        if frame.input_snapshot.to_lowercase().contains("json") {
            let output_trimmed = frame.predicted_output.trim();
            if output_trimmed.starts_with('{') || output_trimmed.starts_with('[') {
                score += 0.1;
                notes.push("Format JSON détecté (+0.1)");
            } else {
                score -= 0.3;
                notes.push("Format JSON attendu mais non détecté (-0.3)");
            }
        }

        // Normalisation (Clamp) entre 0.0 et 1.0 pour rester cohérent
        score = score.clamp(0.0, 1.0);

        CritiqueResult {
            score,
            is_acceptable: score >= self.threshold,
            reasoning: if notes.is_empty() {
                "Évaluation standard (Pas de critères spécifiques détectés).".into()
            } else {
                notes.join(", ")
            },
        }
    }
}
