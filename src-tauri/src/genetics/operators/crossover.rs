use rand::prelude::*;

pub fn single_point_crossover<T: Clone>(
    parent1: &[T],
    parent2: &[T],
    rng: &mut dyn RngCore,
) -> Vec<T> {
    assert_eq!(parent1.len(), parent2.len(), "Parent size mismatch");
    let len = parent1.len();
    if len == 0 {
        return vec![];
    }

    let split_idx = rng.random_range(1..len); // UPDATE

    let mut child = Vec::with_capacity(len);
    child.extend_from_slice(&parent1[..split_idx]);
    child.extend_from_slice(&parent2[split_idx..]);

    child
}

pub fn uniform_crossover<T: Clone>(parent1: &[T], parent2: &[T], rng: &mut dyn RngCore) -> Vec<T> {
    assert_eq!(parent1.len(), parent2.len(), "Parent size mismatch");

    parent1
        .iter()
        .zip(parent2.iter())
        .map(|(g1, g2)| {
            if rng.random_bool(0.5) {
                g1.clone()
            } else {
                g2.clone()
            } // UPDATE
        })
        .collect()
}

pub fn sbx_crossover(p1: f32, p2: f32, eta: f32, rng: &mut dyn RngCore) -> (f32, f32) {
    if rng.random::<f32>() > 0.5 {
        // UPDATE
        return (p1, p2);
    }

    let u: f32 = rng.random(); // UPDATE
    let beta = if u <= 0.5 {
        (2.0 * u).powf(1.0 / (eta + 1.0))
    } else {
        (1.0 / (2.0 * (1.0 - u))).powf(1.0 / (eta + 1.0))
    };

    let c1 = 0.5 * ((1.0 + beta) * p1 + (1.0 - beta) * p2);
    let c2 = 0.5 * ((1.0 - beta) * p1 + (1.0 + beta) * p2);

    (c1, c2)
}

// --- Tests Unitaires ---
#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_single_point() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let p1 = vec![1, 1, 1, 1];
        let p2 = vec![2, 2, 2, 2];

        let child = single_point_crossover(&p1, &p2, &mut rng);

        assert_eq!(child.len(), 4);
        // Le début vient de p1, la fin de p2
        // Avec seed 42, split_idx est souvent prévisible, mais on vérifie juste le mélange
        assert!(child.contains(&1));
        assert!(child.contains(&2));
    }

    #[test]
    fn test_uniform_crossover() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(123);
        let p1 = vec![1, 1, 1, 1, 1];
        let p2 = vec![2, 2, 2, 2, 2];

        let child = uniform_crossover(&p1, &p2, &mut rng);

        // On s'attend à un mélange statistique
        let count_1 = child.iter().filter(|&&x| x == 1).count();
        let count_2 = child.iter().filter(|&&x| x == 2).count();

        assert!(count_1 > 0);
        assert!(count_2 > 0);
        assert_eq!(count_1 + count_2, 5);
    }
}
