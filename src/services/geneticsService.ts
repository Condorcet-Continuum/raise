import { invoke } from '@tauri-apps/api/core';

export interface GeneticsParams {
  population_size: number;
  generations: number;
  mutation_rate: number;
}

export interface OptimizationResult {
  best_score: number;
  duration_ms: number;
  improvement_log: number[];
  best_candidate_id: string;
}

class GeneticsService {
  async runOptimization(params: GeneticsParams, model: any): Promise<OptimizationResult> {
    try {
      // Les clés de params sont transmises telles quelles au backend Rust
      return await invoke<OptimizationResult>('run_genetic_optimization', {
        params,
        model,
      });
    } catch (error) {
      console.error('❌ Erreur génétique:', error);
      throw error;
    }
  }
}

export const geneticsService = new GeneticsService();
