// FICHIER : src/types/spatial.types.ts

/**
 * Enumération des couches architecturales.
 * Doit rester synchronisée avec l'enum Rust LayerType.
 */
export enum LayerType {
  OA = 0, // Operational Analysis
  SA = 1, // System Analysis
  LA = 2, // Logical Architecture
  PA = 3, // Physical Architecture
  Chaos = 4, // Zone non structurée / IA
}

/**
 * Représentation d'un nœud dans l'espace 3D.
 * Miroir de Rust::SpatialNode
 */
export interface SpatialNode {
  id: string;
  label: string;
  /** Coordonnées [x, y, z] optimisées pour le GPU */
  position: [number, number, number];
  layer: LayerType;
  weight: number;
  /** Facteur de stabilité : 0.0 (Instable/Vibrant) -> 1.0 (Stable/Fixe) */
  stability: number;
}

/**
 * Lien entre deux nœuds.
 * Miroir de Rust::SpatialLink
 */
export interface SpatialLink {
  source: string;
  target: string;
  strength: number; // Opacité ou épaisseur du lien
}

/**
 * Métadonnées globales du graphe.
 * Miroir de Rust::GraphMeta
 */
export interface GraphMeta {
  node_count: number;
  /** Nombre de nœuds par couche (indexé par l'enum LayerType) */
  layer_distribution: [number, number, number, number, number];
}

/**
 * Charge utile complète renvoyée par la commande 'get_spatial_topology'.
 * Miroir de Rust::SpatialGraph
 */
export interface SpatialGraph {
  nodes: SpatialNode[];
  links: SpatialLink[];
  meta: GraphMeta;
}
