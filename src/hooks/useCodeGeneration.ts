import { useState, useCallback } from 'react';
import { codegenService } from '@/services/codegenService';
import { useModelStore } from '@/store/model-store';

export function useCodeGeneration() {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [generatedCode, setGeneratedCode] = useState<string>('');

  // On récupère le projet actuel depuis le store global
  const currentProject = useModelStore((state) => state.project);

  const generate = useCallback(
    async (language: string) => {
      if (!currentProject) {
        setError("Aucun projet n'est chargé dans le contexte.");
        return;
      }

      setLoading(true);
      setError(null);
      setGeneratedCode('');

      try {
        // Appel au service (qui peut appeler Rust ou faire le traitement en JS)
        const result = await codegenService.generateCode(language, currentProject);
        setGeneratedCode(result);
      } catch (err: any) {
        console.error('[useCodeGeneration] Error:', err);
        setError(err.message || String(err));
      } finally {
        setLoading(false);
      }
    },
    [currentProject],
  );

  const copyToClipboard = useCallback(async () => {
    if (!generatedCode) return false;
    try {
      await navigator.clipboard.writeText(generatedCode);
      return true;
    } catch (e) {
      console.error('Copy failed', e);
      return false;
    }
  }, [generatedCode]);

  return {
    loading,
    error,
    generatedCode,
    generate,
    copyToClipboard,
  };
}
