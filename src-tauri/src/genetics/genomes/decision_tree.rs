use crate::genetics::traits::Genome;
use crate::utils::prelude::*;
use rand::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TreeNode {
    Internal {
        feature_index: usize,
        threshold: f32,
        left: Box<TreeNode>,
        right: Box<TreeNode>,
    },
    Leaf {
        value: f32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecisionTreeGenome {
    pub root: TreeNode,
    pub max_depth: usize,
    pub num_features: usize,
}

impl DecisionTreeGenome {
    pub fn new_random(depth: usize, num_features: usize) -> Self {
        let mut rng = rand::rng();
        Self {
            root: Self::generate_random_node(&mut rng, depth, num_features),
            max_depth: depth,
            num_features,
        }
    }

    fn generate_random_node(rng: &mut ThreadRng, depth: usize, num_features: usize) -> TreeNode {
        if depth == 0 || rng.random_bool(0.1) {
            TreeNode::Leaf {
                value: rng.random_range(0.0..1.0),
            }
        } else {
            TreeNode::Internal {
                feature_index: rng.random_range(0..num_features),
                threshold: rng.random_range(-1.0..1.0),
                left: Box::new(Self::generate_random_node(rng, depth - 1, num_features)),
                right: Box::new(Self::generate_random_node(rng, depth - 1, num_features)),
            }
        }
    }

    pub fn size(&self) -> usize {
        self.root.size()
    }
}

impl TreeNode {
    pub fn size(&self) -> usize {
        match self {
            TreeNode::Leaf { .. } => 1,
            TreeNode::Internal { left, right, .. } => 1 + left.size() + right.size(),
        }
    }

    pub fn get_node_mut(
        &mut self,
        target_idx: usize,
        current_idx: &mut usize,
    ) -> Option<&mut TreeNode> {
        if *current_idx == target_idx {
            return Some(self);
        }
        *current_idx += 1;

        match self {
            TreeNode::Internal { left, right, .. } => {
                if let Some(node) = left.get_node_mut(target_idx, current_idx) {
                    return Some(node);
                }
                right.get_node_mut(target_idx, current_idx)
            }
            TreeNode::Leaf { .. } => None,
        }
    }

    pub fn get_node(&self, target_idx: usize, current_idx: &mut usize) -> Option<TreeNode> {
        if *current_idx == target_idx {
            return Some(self.clone());
        }
        *current_idx += 1;

        match self {
            TreeNode::Internal { left, right, .. } => {
                if let Some(node) = left.get_node(target_idx, current_idx) {
                    return Some(node);
                }
                right.get_node(target_idx, current_idx)
            }
            TreeNode::Leaf { .. } => None,
        }
    }
}

impl Genome for DecisionTreeGenome {
    fn random() -> Self {
        Self::new_random(3, 2)
    }

    fn mutate(&mut self, rate: f32) {
        let mut rng = rand::rng();
        if rng.random::<f32>() > rate {
            return;
        }

        let size = self.root.size();
        let target = rng.random_range(0..size);

        let mut current_idx = 0;
        if let Some(node) = self.root.get_node_mut(target, &mut current_idx) {
            if rng.random_bool(0.5) {
                match node {
                    TreeNode::Internal {
                        feature_index,
                        threshold,
                        ..
                    } => {
                        if rng.random_bool(0.5) {
                            *threshold += rng.random_range(-0.1..0.1);
                        } else {
                            *feature_index = rng.random_range(0..self.num_features);
                        }
                    }
                    TreeNode::Leaf { value } => {
                        *value += rng.random_range(-0.1..0.1);
                    }
                }
            } else {
                *node = Self::generate_random_node(&mut rng, 2, self.num_features);
            }
        }
    }

    fn crossover(&self, other: &Self) -> Self {
        let mut rng = rand::rng();
        let mut child = self.clone();

        let size_p1 = child.size();
        let point_p1 = rng.random_range(0..size_p1);

        let size_p2 = other.size();
        let point_p2 = rng.random_range(0..size_p2);

        let mut idx_p2 = 0;
        let subtree_p2 = other.root.get_node(point_p2, &mut idx_p2);

        if let Some(subtree) = subtree_p2 {
            let mut idx_p1 = 0;
            if let Some(target_node) = child.root.get_node_mut(point_p1, &mut idx_p1) {
                *target_node = subtree;
            }
        }

        child
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_generation() {
        let genome = DecisionTreeGenome::new_random(3, 5);
        assert!(genome.size() >= 1);
        assert_eq!(genome.num_features, 5);
    }

    #[test]
    fn test_tree_mutation() {
        let mut genome = DecisionTreeGenome::new_random(5, 2);
        // CORRECTION : Variable supprimée car inutilisée
        // let original_size = genome.size();

        for _ in 0..10 {
            genome.mutate(1.0);
        }

        assert!(genome.size() > 0);
    }

    #[test]
    fn test_tree_crossover() {
        let p1 = DecisionTreeGenome::new_random(3, 2);
        let p2 = DecisionTreeGenome::new_random(3, 2);

        let child = p1.crossover(&p2);

        assert!(child.size() > 0);
        assert_eq!(child.num_features, 2);
    }
}
