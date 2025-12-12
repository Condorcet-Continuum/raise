import { invoke } from '@tauri-apps/api/core';
import { useSettingsStore } from '@/store/settings-store';
import type { ProjectModel } from '@/types/model.types'; // Assurez-vous que ce type existe ou utilisez 'any' temporairement

export class ModelService {
  /**
   * Charge le modèle complet depuis le backend Rust.
   * Utilise la configuration active (Space/DB) si aucun paramètre n'est fourni.
   */
  async loadProjectModel(space?: string, db?: string): Promise<ProjectModel> {
    const settings = useSettingsStore.getState();
    const targetSpace = space || settings.jsonDbSpace;
    const targetDb = db || settings.jsonDbDatabase;

    try {
      console.log(`[ModelService] Loading project from ${targetSpace}/${targetDb}...`);
      const start = performance.now();

      // Appel à la commande Rust 'load_project_model'
      const model = await invoke<ProjectModel>('load_project_model', {
        space: targetSpace,
        db: targetDb,
      });

      const duration = (performance.now() - start).toFixed(0);
      const count = model.meta?.elementCount || 'N/A'; // Utilisation safe de meta
      console.log(`[ModelService] Loaded ${count} elements in ${duration}ms`);

      return model;
    } catch (error) {
      console.error('[ModelService] Failed to load project:', error);
      throw error;
    }
  }

  /**
   * Sauvegarde le modèle (si le backend le supporte).
   */
  async saveProjectModel(model: ProjectModel): Promise<void> {
    const { jsonDbSpace, jsonDbDatabase } = useSettingsStore.getState();
    try {
      await invoke('save_project_model', {
        space: jsonDbSpace,
        db: jsonDbDatabase,
        model,
      });
    } catch (error) {
      console.error('[ModelService] Save failed:', error);
      throw error;
    }
  }
}

export const modelService = new ModelService();
