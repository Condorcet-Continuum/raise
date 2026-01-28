use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

// --- DÉFINITION DES TYPES ---

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum LayerType {
    OA = 0,    // Operational Analysis
    SA = 1,    // System Analysis
    LA = 2,    // Logical Architecture
    PA = 3,    // Physical Architecture
    Chaos = 4, // Zone IA / Non-structurée
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpatialNode {
    pub id: String,
    pub label: String,
    pub position: [f32; 3], // [x, y, z] Optimisé GPU
    pub layer: LayerType,
    pub weight: f32,
    pub stability: f32, // 0.0 (Vibration) -> 1.0 (Stable)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpatialLink {
    pub source: String,
    pub target: String,
    pub strength: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpatialGraph {
    pub nodes: Vec<SpatialNode>,
    pub links: Vec<SpatialLink>,
    pub meta: GraphMeta,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraphMeta {
    pub node_count: usize,
    pub layer_distribution: [usize; 5],
}

// --- LOGIQUE MÉTIER ---

#[tauri::command]
pub fn get_spatial_topology() -> SpatialGraph {
    let mut nodes = Vec::new();
    let mut links = Vec::new();
    let mut layer_counts = [0; 5];

    let layers = [LayerType::OA, LayerType::SA, LayerType::LA, LayerType::PA];

    // Génération procédurale de l'architecture
    for (i, layer) in layers.iter().enumerate() {
        let y_pos = (3 - i as i32) as f32 * 10.0;
        let root_id = format!("root_{:?}", layer);

        // Nœud Pilier
        nodes.push(SpatialNode {
            id: root_id.clone(),
            label: format!("Layer {:?}", layer),
            position: [0.0, y_pos, 0.0],
            layer: layer.clone(),
            weight: 2.0,
            stability: 1.0,
        });
        layer_counts[layer.clone() as usize] += 1;

        // Composants Satellites
        let satellite_count = 6 + i * 2;
        for j in 0..satellite_count {
            let angle = (j as f32 / satellite_count as f32) * 2.0 * PI;
            let radius = 6.0 + (i as f32 * 2.0);
            let sub_id = format!("node_{}_{}", i, j);

            nodes.push(SpatialNode {
                id: sub_id.clone(),
                label: format!("Sys-{}-{}", i, j),
                position: [
                    radius * angle.cos(),
                    y_pos + (if j % 2 == 0 { 0.5 } else { -0.5 }),
                    radius * angle.sin(),
                ],
                layer: layer.clone(),
                weight: 1.0,
                stability: if j % 3 == 0 { 0.4 } else { 0.98 },
            });
            layer_counts[layer.clone() as usize] += 1;

            links.push(SpatialLink {
                source: root_id.clone(),
                target: sub_id,
                strength: 0.7,
            });
        }
    }

    // CORRECTION : On capture la taille AVANT de déplacer le vecteur 'nodes'
    let final_node_count = nodes.len();

    SpatialGraph {
        nodes, // Ici 'nodes' est déplacé (moved)
        links,
        meta: GraphMeta {
            node_count: final_node_count, // On utilise la valeur capturée
            layer_distribution: layer_counts,
        },
    }
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topology_generation_integrity() {
        let graph = get_spatial_topology();
        assert!(graph.nodes.len() > 0);
        assert!(graph.links.len() > 0);
        assert_eq!(graph.meta.layer_distribution.len(), 5);
        assert_eq!(graph.meta.node_count, graph.nodes.len());
    }
}
