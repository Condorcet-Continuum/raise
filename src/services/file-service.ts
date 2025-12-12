import { open, save } from '@tauri-apps/plugin-dialog';
import { readTextFile, writeTextFile } from '@tauri-apps/plugin-fs';

/**
 * Service de gestion de fichiers locaux (Ouvrir / Sauvegarder).
 * Utilise les boîtes de dialogue natives du système d'exploitation.
 */
class FileService {
  /**
   * Ouvre une boîte de dialogue pour sélectionner un fichier JSON.
   * Retourne le contenu parsé ou null si annulé.
   */
  async openJsonFile<T = any>(): Promise<T | null> {
    try {
      const selectedPath = await open({
        multiple: false,
        filters: [{ name: 'JSON Model', extensions: ['json', 'aird'] }],
      });

      if (!selectedPath) return null;

      // Lecture du fichier sélectionné
      const content = await readTextFile(selectedPath as string);
      return JSON.parse(content) as T;
    } catch (err) {
      console.error('File open error:', err);
      throw err;
    }
  }

  /**
   * Ouvre une boîte de dialogue pour sauvegarder un objet en JSON.
   */
  async saveJsonFile(data: any, suggestedName = 'model.json'): Promise<void> {
    try {
      const savePath = await save({
        defaultPath: suggestedName,
        filters: [{ name: 'JSON Model', extensions: ['json'] }],
      });

      if (!savePath) return;

      await writeTextFile(savePath, JSON.stringify(data, null, 2));
    } catch (err) {
      console.error('File save error:', err);
      throw err;
    }
  }
}

export const fileService = new FileService();
