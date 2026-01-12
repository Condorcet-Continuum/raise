use candle_core::{Device, Result, Tensor};
use candle_nn::{Linear, Module, VarMap};

pub struct LoraLinear {
    old_linear: Linear,
    pub lora_a: Tensor, // Projection : [Out, Rank]
    pub lora_b: Tensor, // Réduction  : [Rank, In]
    scale: f64,
}

impl LoraLinear {
    pub fn new(
        old_linear: Linear,
        rank: usize,
        alpha: f64,
        varmap: &mut VarMap,
        device: &Device,
    ) -> Result<Self> {
        let (out_dims, in_dims) = old_linear.weight().shape().dims2()?;
        let dtype = old_linear.weight().dtype();

        // lora_a : [Out, Rank]
        let lora_a = varmap.get(
            (out_dims, rank),
            "lora_a",
            candle_nn::init::DEFAULT_KAIMING_NORMAL,
            dtype,
            device,
        )?;
        // lora_b : [Rank, In]
        let lora_b = varmap.get(
            (rank, in_dims),
            "lora_b",
            candle_nn::init::ZERO,
            dtype,
            device,
        )?;

        let scale = alpha / rank as f64;

        Ok(Self {
            old_linear,
            lora_a,
            lora_b,
            scale,
        })
    }
}

impl Module for LoraLinear {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        // Calcul standard
        let standard_output = self.old_linear.forward(x)?;

        // Calcul LoRA corrigé :
        // 1. x [1, In] * lora_b^T [In, Rank] -> [1, Rank]
        // 2. [1, Rank] * lora_a^T [Rank, Out] -> [1, Out]
        let lora_output = x
            .matmul(&self.lora_b.t()?)? // Réduction vers le rang
            .matmul(&self.lora_a.t()?)?; // Projection vers la sortie

        standard_output.add(&(lora_output * self.scale)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::VarMap;

    #[test]
    fn test_lora_linear_forward_shape() -> Result<()> {
        let device = Device::Cpu;
        let mut varmap = VarMap::new();

        // Simule une couche 10 (In) -> 20 (Out)
        let weight = Tensor::zeros((20, 10), DType::F32, &device)?;
        let bias = Tensor::zeros(20, DType::F32, &device)?;
        let linear = Linear::new(weight, Some(bias));

        let lora = LoraLinear::new(linear, 4, 1.0, &mut varmap, &device)?;

        // Input [1, 10]
        let input = Tensor::ones((1, 10), DType::F32, &device)?;
        let output = lora.forward(&input)?;

        // Output doit être [1, 20]
        assert_eq!(output.shape().dims(), &[1, 20]);
        Ok(())
    }
}
