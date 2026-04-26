// FICHIER : src-tauri/src/ai/graph_store/logic.rs

use crate::utils::prelude::*;

pub struct ArcadiaLogic {
    /// Matrice de violation [N, N] (1.0 si lien interdit, 0.0 sinon)
    pub violation_matrix: NeuralTensor,
}

impl ArcadiaLogic {
    /// 🎯 CONSTRUCTION DE LA MATRICE DE VIOLATION
    pub fn build_violation_matrix(
        index_to_uri: &[String],
        device: &ComputeHardware,
    ) -> RaiseResult<Self> {
        let n = index_to_uri.len();
        if n == 0 {
            raise_error!("ERR_GNN_LOGIC_EMPTY", error = "Index d'URIs vide.");
        }

        let mut mask = Vec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                let src_type = Self::extract_type(&index_to_uri[i]);
                let dst_type = Self::extract_type(&index_to_uri[j]);

                if Self::is_arcadia_violation(src_type, dst_type) {
                    mask.push(1.0f32);
                } else {
                    mask.push(0.0f32);
                }
            }
        }

        let tensor = match NeuralTensor::from_vec(mask, (n, n), device) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_GNN_LOGIC_TENSOR_FAILED", error = e.to_string()),
        };

        Ok(Self {
            violation_matrix: tensor,
        })
    }

    /// 🎯 CALCUL DE LA PERTE LOGIQUE
    pub fn compute_logic_loss(
        &self,
        predictions: &NeuralTensor,
        lambda: f32,
    ) -> RaiseResult<NeuralTensor> {
        let forbidden_activations = match predictions.mul(&self.violation_matrix) {
            Ok(res) => res,
            Err(e) => raise_error!("ERR_GNN_LOGIC_MUL_FAILED", error = e.to_string()),
        };

        let total_violation = match forbidden_activations.sum_all() {
            Ok(s) => s,
            Err(e) => raise_error!("ERR_GNN_LOGIC_SUM_FAILED", error = e.to_string()),
        };

        let device = total_violation.device();

        // 🎯 FIX : On transforme le tenseur [1] en un scalaire [] pour correspondre à total_violation
        let lambda_tensor = match NeuralTensor::new(&[lambda], device) {
            Ok(t) => match t.reshape(&[]) {
                // On "aplatit" pour obtenir un rang 0
                Ok(r) => r,
                Err(e) => raise_error!("ERR_GNN_LOGIC_RESHAPE", error = e.to_string()),
            },
            Err(e) => raise_error!("ERR_GNN_LOGIC_LAMBDA_ALLOC", error = e.to_string()),
        };

        match total_violation.mul(&lambda_tensor) {
            Ok(l) => Ok(l),
            Err(e) => raise_error!(
                "ERR_GNN_LOGIC_SCALE_FAILED",
                error = e.to_string(),
                context = json_value!({ "action": "COMPUTE_LOGIC_LOSS" })
            ),
        }
    }

    fn extract_type(uri: &str) -> &str {
        uri.split(':').next().unwrap_or("unknown")
    }

    fn is_arcadia_violation(src: &str, dst: &str) -> bool {
        match (src, dst) {
            // Règles de violation (Cycle en V inversé)
            ("pa", "oa") | ("la", "pa") | ("sa", "oa") => true,
            _ => false,
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_logic_loss_calculation() -> RaiseResult<()> {
        let device = ComputeHardware::Cpu;
        let uris = vec!["la:C1".to_string(), "pa:P1".to_string()];
        let logic = ArcadiaLogic::build_violation_matrix(&uris, &device)?;

        let pred_raw = vec![0.0f32, 0.9, 0.0, 0.0];

        // 🎯 FIX : Utilisation directe de raise_error! dans le match
        let predictions = match NeuralTensor::from_vec(pred_raw, (2, 2), &device) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_TEST_TENSOR", error = e.to_string()),
        };

        let loss_tensor = logic.compute_logic_loss(&predictions, 100.0)?;

        let loss_value = match loss_tensor.to_scalar::<f32>() {
            Ok(v) => v,
            Err(e) => raise_error!("ERR_TEST_SCALAR", error = e.to_string()),
        };

        assert!((loss_value - 90.0).abs() < 0.001);
        Ok(())
    }
}
