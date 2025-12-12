// FICHIER : src/types/cognitive.types.ts

// Définition d'un élément minimal pour l'analyse cognitive
export interface ModelElement {
  // id est souvent la clé du Record, mais peut être présent ici aussi
  id?: string;
  name: string;
  kind: string; // ex: "LogicalComponent"
  properties: Record<string, string>;
}

// Le Modèle Cognitif complet (payload envoyé au WASM/Rust)
export interface CognitiveModel {
  id: string;
  elements: Record<string, ModelElement>;
  metadata: Record<string, string>;
}

// Le Rapport d'Analyse (réponse reçue)
export interface AnalysisReport {
  block_id: string;
  status: 'Success' | 'Warning' | 'Failure';
  messages: string[];
  timestamp: number;
  score?: number; // Score de consistance (0-100)
  suggestions?: string[];
}
