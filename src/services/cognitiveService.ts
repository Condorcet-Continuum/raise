import { invoke } from '@tauri-apps/api/core';
// ✅ IMPORT CENTRALISÉ (On supprime les interfaces locales)
import type { AnalysisReport, CognitiveModel } from '@/types/cognitive.types';

class CognitiveService {
  /**
   * Envoie un modèle au moteur de plugins pour analyse via WASM (côté Rust).
   */
  async runConsistencyCheck(model: CognitiveModel): Promise<AnalysisReport> {
    try {
      console.log('📤 Envoi du modèle au bloc cognitif...', model);

      // Le backend attend "modelJson" (camelCase coté JS) -> "model_json" (snake_case coté Rust)
      const jsonString = await invoke<string>('run_consistency_analysis', {
        modelJson: model,
      });

      // Le backend renvoie une string JSON qu'on parse
      const report: AnalysisReport = JSON.parse(jsonString);

      return report;
    } catch (error) {
      console.error('❌ Erreur service cognitif:', error);
      throw error;
    }
  }
}

export const cognitiveService = new CognitiveService();
