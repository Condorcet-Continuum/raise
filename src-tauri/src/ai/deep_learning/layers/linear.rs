use candle_core::{Result, Tensor};

/// Couche entièrement connectée (Dense / Linear).
pub struct Linear {
    pub weight: Tensor,
    pub bias: Option<Tensor>,
}

impl Linear {
    pub fn new(weight: Tensor, bias: Option<Tensor>) -> Self {
        Self { weight, bias }
    }

    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let w_t = self.weight.t()?;
        let mut output = x.matmul(&w_t)?;

        if let Some(bias) = &self.bias {
            output = output.broadcast_add(bias)?;
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device; // DType retiré ici s'il n'est pas utilisé, ou gardé si nécessaire pour les tests

    #[test]
    fn test_linear_shapes() -> Result<()> {
        let device = Device::Cpu;
        let input = Tensor::randn(0f32, 1.0, (2, 3), &device)?;
        let weight = Tensor::randn(0f32, 1.0, (4, 3), &device)?;
        let bias = Tensor::randn(0f32, 1.0, (4,), &device)?;

        let layer = Linear::new(weight, Some(bias));
        let output = layer.forward(&input)?;

        assert_eq!(output.dims(), &[2, 4]);
        Ok(())
    }

    #[test]
    fn test_linear_values() -> Result<()> {
        let device = Device::Cpu;
        let input = Tensor::from_slice(&[1f32, 2f32], (1, 2), &device)?;
        let weight = Tensor::from_slice(&[0.5f32, 0.5f32], (1, 2), &device)?;
        let bias = Tensor::from_slice(&[1f32], (1,), &device)?;

        let layer = Linear::new(weight, Some(bias));
        let output = layer.forward(&input)?;

        let val = output.to_vec2::<f32>()?;
        assert_eq!(val[0][0], 2.5);
        Ok(())
    }
}
