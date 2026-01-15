// FICHIER : src/services/deepLearningService.ts
import { invoke } from '@tauri-apps/api/core'; // Ou "@tauri-apps/api/tauri" selon votre version

export interface DLModelConfig {
  inputDim: number;
  hiddenDim: number;
  outputDim: number;
}

export const deepLearningService = {
  /**
   * Initialise un nouveau modèle LSTM vierge en mémoire (Backend).
   */
  async initModel(config: DLModelConfig): Promise<string> {
    try {
      return await invoke('init_dl_model', {
        inputDim: config.inputDim,
        hiddenDim: config.hiddenDim,
        outputDim: config.outputDim,
      });
    } catch (error) {
      console.error('Erreur initModel:', error);
      throw error;
    }
  },

  /**
   * Effectue une prédiction sur une séquence donnée.
   * @param inputSequence - Vecteur de nombres (f32)
   */
  async predict(inputSequence: number[]): Promise<number[]> {
    try {
      return await invoke('run_dl_prediction', {
        inputSequence,
      });
    } catch (error) {
      console.error('Erreur predict:', error);
      throw error;
    }
  },

  /**
   * Effectue un pas d'entraînement (SGD).
   * @param inputSequence - Données d'entrée
   * @param targetClass - Classe attendue (ex: 0 ou 1 pour binaire)
   * @returns La valeur de la perte (Loss) après ce pas.
   */
  async trainStep(inputSequence: number[], targetClass: number): Promise<number> {
    try {
      return await invoke('train_dl_step', {
        inputSequence,
        targetClass,
      });
    } catch (error) {
      console.error('Erreur trainStep:', error);
      throw error;
    }
  },

  /**
   * Sauvegarde le modèle actuel dans un fichier .safetensors
   */
  async saveModel(path: string): Promise<string> {
    try {
      return await invoke('save_dl_model', { path });
    } catch (error) {
      console.error('Erreur saveModel:', error);
      throw error;
    }
  },

  /**
   * Charge un modèle depuis le disque.
   */
  async loadModel(path: string, config: DLModelConfig): Promise<string> {
    try {
      return await invoke('load_dl_model', {
        path,
        inputDim: config.inputDim,
        hiddenDim: config.hiddenDim,
        outputDim: config.outputDim,
      });
    } catch (error) {
      console.error('Erreur loadModel:', error);
      throw error;
    }
  },
};
