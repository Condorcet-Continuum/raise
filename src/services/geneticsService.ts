import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export interface FunctionInfo {
  id: string;
  load: number;
}
export interface ComponentInfo {
  id: string;
  capacity: number;
}
export interface DataFlowInfo {
  source_id: string;
  target_id: string;
  volume: number;
}

export interface OptimizationRequest {
  population_size: number;
  max_generations: number;
  mutation_rate: number;
  crossover_rate: number;
  functions: FunctionInfo[];
  components: ComponentInfo[];
  flows: DataFlowInfo[];
}

export interface OptimizationProgress {
  generation: number;
  best_fitness: number[];
  diversity: number;
}

export interface OptimizationResult {
  duration_ms: number;
  pareto_front: {
    fitness: number[];
    constraint_violation: number;
    allocation: [string, string][];
  }[];
}

class GeneticsService {
  async runArchitectureOptimization(
    params: OptimizationRequest,
    onProgress?: (progress: OptimizationProgress) => void,
  ): Promise<OptimizationResult> {
    let unlistenProgress: UnlistenFn | undefined;
    try {
      if (onProgress) {
        unlistenProgress = await listen<OptimizationProgress>('genetics://progress', (event) =>
          onProgress(event.payload),
        );
      }
      return await invoke<OptimizationResult>('run_architecture_optimization', { params });
    } finally {
      if (unlistenProgress) unlistenProgress();
    }
  }
}

export const geneticsService = new GeneticsService();
