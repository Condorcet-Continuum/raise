use rand::prelude::*;
use rand_distr::{Distribution, Normal};

pub fn swap_mutation<T>(genes: &mut [T], mutation_rate: f32, rng: &mut dyn Rng) {
    if genes.len() < 2 {
        return;
    }

    if rng.random::<f32>() < mutation_rate {
        let idx1 = rng.random_range(0..genes.len());
        let idx2 = rng.random_range(0..genes.len());
        genes.swap(idx1, idx2);
    }
}

pub fn uniform_mutation<T, F>(genes: &mut [T], mutation_rate: f32, rng: &mut dyn Rng, sampler: F)
where
    F: Fn(&mut dyn Rng) -> T,
{
    for gene in genes.iter_mut() {
        if rng.random::<f32>() < mutation_rate {
            *gene = sampler(rng);
        }
    }
}

pub fn gaussian_mutation(genes: &mut [f32], mutation_rate: f32, sigma: f32, rng: &mut dyn Rng) {
    let normal = Normal::new(0.0, sigma).unwrap();
    for gene in genes.iter_mut() {
        if rng.random::<f32>() < mutation_rate {
            *gene += normal.sample(rng);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_swap_mutation() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let mut genes = vec![1, 2, 3, 4, 5];
        let original = genes.clone();

        // Taux de 1.0 = swap forcé
        swap_mutation(&mut genes, 1.0, &mut rng);

        assert_ne!(genes, original, "Le vecteur doit avoir changé");
        assert_eq!(genes.len(), original.len());
        // CORRECTION ICI : sum::<i32>() explicite des deux côtés
        assert_eq!(genes.iter().sum::<i32>(), original.iter().sum::<i32>());
    }

    #[test]
    fn test_uniform_mutation() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let mut genes = vec![0, 0, 0, 0, 0];

        uniform_mutation(&mut genes, 1.0, &mut rng, |_| 1);

        assert_eq!(genes, vec![1, 1, 1, 1, 1]);
    }

    #[test]
    fn test_gaussian_mutation() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let mut genes = vec![10.0, 10.0];

        gaussian_mutation(&mut genes, 1.0, 0.1, &mut rng);

        assert_ne!(genes[0], 10.0);
        assert!((genes[0] - 10.0).abs() < 1.0);
    }
}
