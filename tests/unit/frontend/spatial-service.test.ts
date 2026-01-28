// FICHIER : tests/unit/frontend/spatial-service.test.ts

import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest';
import { spatialService } from '../../../src/services/spatial-service';
import { CMDS } from '../../../src/services/tauri-commands';

// Mock du module Tauri pour intercepter les appels 'invoke'
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';

describe('SpatialService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should call the correct Tauri command and return data', async () => {
    // Données simulées renvoyées par le backend fictif
    const mockGraph = {
      nodes: [
        { id: 'n1', label: 'Node 1', position: [0, 10, 0], layer: 0, weight: 1, stability: 1 },
      ],
      links: [],
      meta: { node_count: 1, layer_distribution: [1, 0, 0, 0, 0] },
    };

    // CORRECTION : On type explicitement le mock au lieu d'utiliser 'any'
    (invoke as Mock).mockResolvedValue(mockGraph);

    // Action
    const result = await spatialService.getTopology();

    // Vérifications
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith(CMDS.SPATIAL_TOPOLOGY);
    expect(result).toEqual(mockGraph);
    expect(result.nodes.length).toBe(1);
  });

  it('should throw an error if the backend fails', async () => {
    // Simulation d'une erreur Rust
    const errorMsg = 'Rust Backend Error';
    (invoke as Mock).mockRejectedValue(new Error(errorMsg));

    // Vérifie que l'erreur remonte bien jusqu'au composant
    await expect(spatialService.getTopology()).rejects.toThrow(errorMsg);
  });

  it('should validate data integrity', async () => {
    // Simulation d'une réponse invalide (ex: null)
    (invoke as Mock).mockResolvedValue(null);

    await expect(spatialService.getTopology()).rejects.toThrow('Invalid topology data');
  });
});
