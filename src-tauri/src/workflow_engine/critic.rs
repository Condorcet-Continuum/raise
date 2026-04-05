// FICHIER : src-tauri/src/workflow_engine/critic.rs
use crate::utils::prelude::*;

use crate::ai::assurance::xai::XaiFrame;
use crate::json_db::collections::manager::CollectionsManager;
use crate::rules_engine::ast::Expr;
use crate::rules_engine::evaluator::{DataProvider, Evaluator};

/// Résultat de l'évaluation d'une action par le critique
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct CritiqueResult {
    pub score: f32,          // Score de 0.0 à 1.0
    pub is_acceptable: bool, // Verdict binaire (Passe / Passe pas)
    pub reasoning: String,   // Explication pour l'audit
}

/// 🎯 NOUVEAU : Le Provider qui permet au moteur de règles d'interroger la base de données
pub struct CriticDataProvider<'a> {
    manager: &'a CollectionsManager<'a>,
}

#[async_interface]
impl<'a> DataProvider for CriticDataProvider<'a> {
    async fn get_value(&self, collection: &str, id: &str, field: &str) -> Option<JsonValue> {
        // Zéro Dette : On gère l'erreur silencieusement ici car c'est une interface Option
        match self.manager.get_document(collection, id).await {
            Ok(Some(doc)) => doc.get(field).cloned(),
            _ => None,
        }
    }
}

/// Le Critique évalue la qualité des sorties du système via le Moteur de Règles
pub struct WorkflowCritic {
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

    /// Analyse formellement une XaiFrame via l'AST du moteur de règles.
    pub async fn evaluate(
        &self,
        frame: &XaiFrame,
        manager: &CollectionsManager<'_>,
        rules: &[Expr], // Les règles métier injectées (Issues d'un Mandat ou d'une Policy)
    ) -> RaiseResult<CritiqueResult> {
        let output_len = frame.predicted_output.len();
        if output_len == 0 {
            return Ok(CritiqueResult {
                score: 0.0,
                is_acceptable: false,
                reasoning: "La sortie de l'agent est vide.".into(),
            });
        }

        // 1. Transformer la preuve (XaiFrame) en contexte JSON pour le moteur de règles
        let context_data = match crate::utils::json::serialize_to_value(frame) {
            Ok(v) => v,
            Err(e) => raise_error!(
                "ERR_CRITIC_XAI_SERIALIZATION",
                error = e,
                context = json_value!({"xai_id": frame.id})
            ),
        };

        let provider = CriticDataProvider { manager };
        let mut score: f32 = 1.0;
        let mut notes = Vec::new();
        let mut all_acceptable = true;

        // 2. Évaluation déterministe de chaque règle métier
        for (i, rule) in rules.iter().enumerate() {
            let eval_result =
                match Box::pin(Evaluator::evaluate(rule, &context_data, &provider)).await {
                    Ok(res) => res,
                    Err(e) => raise_error!(
                        "ERR_CRITIC_RULE_EVALUATION",
                        error = e,
                        context = json_value!({ "rule_index": i, "xai_id": frame.id })
                    ),
                };

            // On s'attend à ce qu'une règle de qualité retourne un booléen
            match eval_result.as_bool() {
                Some(true) => {
                    notes.push(format!("Règle {} validée", i));
                }
                Some(false) => {
                    score -= 0.2; // Pénalité configurable
                    all_acceptable = false;
                    notes.push(format!("Règle {} violée", i));
                }
                None => raise_error!(
                    "ERR_CRITIC_RULE_NOT_BOOL",
                    context = json_value!({
                        "rule_index": i,
                        "received_type": eval_result,
                        "hint": "Une règle d'assurance qualité doit s'évaluer en un booléen (Vrai/Faux)."
                    })
                ),
            }
        }

        score = score.clamp(0.0, 1.0);

        Ok(CritiqueResult {
            score,
            is_acceptable: score >= self.threshold && all_acceptable,
            reasoning: notes.join(" | "),
        })
    }
}
