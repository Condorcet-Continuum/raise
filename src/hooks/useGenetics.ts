import { useState, useCallback, useMemo } from 'react';
import {
  geneticsService,
  OptimizationRequest,
  OptimizationResult,
  OptimizationProgress,
} from '@/services/geneticsService';
import { useModelStore } from '@/store/model-store';
import { ArcadiaElement } from '@/types/model.types';

export const useGenetics = () => {
  const currentProject = useModelStore((state) => state.project);
  const [loading, setLoading] = useState(false);
  const [progress, setProgress] = useState<OptimizationProgress | null>(null);
  const [history, setHistory] = useState<OptimizationProgress[]>([]);
  const [result, setResult] = useState<OptimizationResult | null>(null);

  const canRun = useMemo(() => {
    // CORRECTION : On vérifie réellement si le projet contient des données
    const hasFunctions = (currentProject?.la?.functions?.length || 0) > 0;
    const hasComponents = (currentProject?.pa?.components?.length || 0) > 0;
    // On retourne true si des données existent, mais le bouton UI pourra être activé par le mode Démo aussi
    return hasFunctions && hasComponents;
  }, [currentProject]);

  const runOptimization = useCallback(
    async (
      config: {
        population_size: number;
        max_generations: number;
        mutation_rate: number;
        crossover_rate: number;
      },
      customRequest?: OptimizationRequest,
    ) => {
      setLoading(true);
      setProgress(null);
      setHistory([]);
      setResult(null);

      let request: OptimizationRequest;

      if (customRequest) {
        console.log("🧪 Utilisation des données fournies par l'UI...");
        request = { ...customRequest, ...config };
      } else {
        const rawFunctions = (currentProject?.la?.functions || []) as ArcadiaElement[];
        const rawComponents = (currentProject?.pa?.components || []) as ArcadiaElement[];
        const rawExchanges = (currentProject?.la?.exchanges || []) as ArcadiaElement[];

        request = {
          ...config,
          functions: rawFunctions.map((f) => {
            const p = (f.properties || {}) as Record<string, unknown>;
            return { id: f.id, load: Math.max(1, Number(p.complexity) || 10) };
          }),
          components: rawComponents.map((c) => {
            const p = (c.properties || {}) as Record<string, unknown>;
            return { id: c.id, capacity: Math.max(1, Number(p.capacity) || 100) };
          }),
          flows: rawExchanges.map((e) => {
            const p = (e.properties || {}) as Record<string, unknown>;
            return {
              source_id: String(p.source || e.source || ''),
              target_id: String(p.target || e.target || ''),
              volume: Math.max(1, Number(p.volume) || 1),
            };
          }),
        };
      }

      try {
        const res = await geneticsService.runArchitectureOptimization(request, (p) => {
          setProgress(p);
          setHistory((prev) => [...prev, p]);
        });
        setResult(res);
        return res;
      } catch (e) {
        console.error(e);
        throw e;
      } finally {
        setLoading(false);
      }
    },
    [currentProject],
  );

  return {
    runOptimization,
    loading,
    progress,
    history,
    result,
    canRun,
    stats: {
      functionsCount: currentProject?.la?.functions?.length || 0,
      componentsCount: currentProject?.pa?.components?.length || 0,
    },
  };
};
