// FICHIER : src-tauri/src/ai/deep_learning/trainer.rs
use crate::utils::prelude::*; // 🎯 Utilisation stricte de la façade RAISE

use crate::ai::deep_learning::models::sequence_net::SequenceNet;
use candle_core::Tensor;
use candle_nn::optim::{AdamW, Optimizer, ParamsAdamW};
use candle_nn::VarMap;

/// Gère l'apprentissage du réseau avec l'optimiseur accéléré AdamW.
/// Intègre la gestion des Momentum et la résilience aux erreurs de calcul.
pub struct Trainer {
    optimizer: AdamW,
}

impl Trainer {
    /// 🎯 Crée un Trainer à partir de la configuration centralisée des points de montage.
    pub fn from_config(varmap: &VarMap, config: &DeepLearningConfig) -> RaiseResult<Self> {
        Self::new(varmap, config.learning_rate)
    }

    /// Constructeur sémantique avec initialisation de l'optimiseur AdamW.
    pub fn new(varmap: &VarMap, learning_rate: f64) -> RaiseResult<Self> {
        let vars = varmap.all_vars();

        // 🛡️ Sécurité : Empêcher l'initialisation sans paramètres
        if vars.is_empty() {
            raise_error!(
                "ERR_OPTIMIZER_INIT",
                error = "Aucune variable trouvée dans le VarMap pour l'entraînement.",
                context = json_value!({ "action": "check_varmap_not_empty" })
            );
        }

        let params = ParamsAdamW {
            lr: learning_rate,
            ..Default::default()
        };
        match AdamW::new(vars, params) {
            Ok(opt) => Ok(Self { optimizer: opt }),
            Err(e) => raise_error!("ERR_OPTIMIZER_INIT", error = e.to_string()),
        }
    }

    /// Exécute un pas d'entraînement complet : Forward -> Loss -> Backward -> Update.
    /// Garantit la mutabilité de l'optimiseur pour la mise à jour des états internes.
    pub fn train_step(
        &mut self,
        model: &SequenceNet,
        input: &Tensor,
        targets: &Tensor,
    ) -> RaiseResult<f64> {
        // 1. Forward Pass sécurisé
        let logits = match model.forward(input) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_TRAINING_FORWARD", error = e.to_string()),
        };

        // 2. Validation et Redimensionnement des tenseurs pour la Cross-Entropy
        let (b, s, v) = match logits.dims3() {
            Ok(dims) => dims,
            Err(e) => raise_error!(
                "ERR_TENSOR_DIMS",
                error = "Dimensions logits invalides (attendu 3D)",
                context = json_value!({"error": e.to_string()})
            ),
        };

        let flat_logits = match logits.reshape((b * s, v)) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_TENSOR_RESHAPE", error = e.to_string()),
        };

        let flat_targets = match targets.reshape(b * s) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_TENSOR_RESHAPE", error = e.to_string()),
        };

        // 3. Calcul de la perte via fonction native optimisée
        let loss = match candle_nn::loss::cross_entropy(&flat_logits, &flat_targets) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_LOSS_CALC", error = e.to_string()),
        };

        // 4. Backward pass & Mise à jour atomique des poids
        match self.optimizer.backward_step(&loss) {
            Ok(_) => (),
            Err(e) => raise_error!("ERR_BACKWARD_STEP", error = e.to_string()),
        };

        // 5. Extraction sécurisée du scalaire pour le monitoring système
        match loss.to_scalar::<f32>() {
            Ok(val) => Ok(val as f64),
            Err(e) => raise_error!("ERR_LOSS_SCALAR", error = e.to_string()),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Rigueur Façade, Convergence & Résilience Matérielle)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{mock, DbSandbox};
    use candle_core::DType;
    use candle_nn::VarBuilder;

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_training_convergence() -> RaiseResult<()> {
        mock::inject_mock_config().await; // 🎯 Alignement config globale
        let sandbox = DbSandbox::new().await;
        let config = &sandbox.config.deep_learning;
        let device = AppConfig::device(); // 🎯 Source unique de vérité pour le matériel

        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, device);

        let model = SequenceNet::new(
            config.input_size,
            config.hidden_size,
            config.output_size,
            vb,
        )?;

        let mut trainer = Trainer::from_config(&varmap, config)?;

        // Création de données synthétiques sur le device configuré
        let input = match Tensor::randn(0f32, 1.0, (1, 1, config.input_size), device) {
            Ok(t) => t,
            Err(e) => return Err(build_error!("ERR_TENSOR_IN", error = e.to_string())),
        };
        let target = match Tensor::zeros((1, 1), DType::U32, device) {
            Ok(t) => t,
            Err(e) => return Err(build_error!("ERR_TENSOR_TGT", error = e.to_string())),
        };

        let initial_loss = trainer.train_step(&model, &input, &target)?;

        let mut final_loss = 0.0;
        for _ in 0..20 {
            final_loss = trainer.train_step(&model, &input, &target)?;
        }

        assert!(
            final_loss < initial_loss,
            "Le modèle n'apprend pas (Loss stable à {})",
            final_loss
        );
        Ok(())
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_resilience_mismatched_dimensions() -> RaiseResult<()> {
        mock::inject_mock_config().await;
        let device = AppConfig::device();

        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, device);

        let model = SequenceNet::new(5, 10, 2, vb)?;
        let mut trainer = Trainer::new(&varmap, 0.01)?;

        // Injection d'une dimension erronée (10 au lieu de 5)
        let bad_input = match Tensor::zeros((1, 1, 10), DType::F32, device) {
            Ok(t) => t,
            Err(e) => return Err(build_error!("ERR_TENSOR_BAD", error = e.to_string())),
        };
        let target = match Tensor::zeros((1, 1), DType::U32, device) {
            Ok(t) => t,
            Err(e) => return Err(build_error!("ERR_TENSOR_BAD", error = e.to_string())),
        };

        let result = trainer.train_step(&model, &bad_input, &target);

        // On vérifie que le moteur intercepte proprement l'erreur au lieu de paniquer
        match result {
            Err(AppError::Structured(data)) => {
                assert!(data.code.contains("ERR_TRAINING_FORWARD"));
                Ok(())
            }
            _ => {
                panic!("Le moteur aurait dû lever une erreur structurée pour dimensions invalides")
            }
        }
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_mount_point_config_loading() -> RaiseResult<()> {
        mock::inject_mock_config().await;
        let sandbox = DbSandbox::new().await;
        let config = &sandbox.config.deep_learning;
        let device = AppConfig::device();

        let varmap = VarMap::new();

        // 🎯 ÉTAPE CRUCIALE : Enregistrer les variables du modèle dans le VarMap
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, device);
        let _model = SequenceNet::new(
            config.input_size,
            config.hidden_size,
            config.output_size,
            vb,
        )?;

        // Maintenant varmap.all_vars() n'est plus vide, l'optimiseur peut démarrer !
        let trainer = Trainer::from_config(&varmap, config);

        assert!(
            trainer.is_ok(),
            "Le Trainer doit pouvoir s'initialiser quand le VarMap contient les variables du modèle"
        );

        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience si les variables sont vides lors de l'initialisation de l'optimiseur.
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_resilience_empty_varmap() -> RaiseResult<()> {
        let varmap = VarMap::new(); // Pas de variables enregistrées dans ce varmap

        // AdamW échouera s'il n'y a aucun paramètre à optimiser
        let trainer = Trainer::new(&varmap, 0.01);

        match trainer {
            Err(AppError::Structured(data)) => {
                assert_eq!(data.code, "ERR_OPTIMIZER_INIT");
                Ok(())
            }
            _ => panic!("L'initialisation aurait dû échouer pour un VarMap vide"),
        }
    }
}
