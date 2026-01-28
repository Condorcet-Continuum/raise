// FICHIER : src/services/spatial-service.ts

import { invoke } from '@tauri-apps/api/core';
import { CMDS } from './tauri-commands';
import type { SpatialGraph } from '../types/spatial.types';

class SpatialService {
  /**
   * Récupère la topologie 3D complète depuis le moteur Rust.
   * Utilise les commandes Tauri définies dans tauri-commands.ts.
   */
  async getTopology(): Promise<SpatialGraph> {
    try {
      console.time('[SpatialService] Fetch Topology');

      // Appel au Backend Rust
      const graph = await invoke<SpatialGraph>(CMDS.SPATIAL_TOPOLOGY);

      console.timeEnd('[SpatialService] Fetch Topology');

      // Vérification basique de l'intégrité des données
      if (!graph || !Array.isArray(graph.nodes)) {
        throw new Error('Invalid topology data received from backend');
      }

      return graph;
    } catch (error) {
      console.error('❌ [SpatialService] Error fetching topology:', error);
      throw error;
    }
  }
}

// Export d'une instance unique (Singleton)
export const spatialService = new SpatialService();
