import { readTextFile, writeTextFile, BaseDirectory } from '@tauri-apps/plugin-fs';

/**
 * Hook utilitaire pour les opérations sur le système de fichiers.
 * Utilise l'API Tauri v2.
 */
export function useFileSystem(baseDir: BaseDirectory = BaseDirectory.AppLocalData) {
  /**
   * Lit un fichier JSON et le parse automatiquement.
   * @param path Chemin relatif au baseDir
   */
  async function readJson<T = unknown>(path: string): Promise<T> {
    try {
      const text = await readTextFile(path, { baseDir });
      return JSON.parse(text) as T;
    } catch (error) {
      console.error(`[useFileSystem] Erreur lecture ${path}:`, error);
      throw error;
    }
  }

  /**
   * Sérialise et écrit un objet dans un fichier JSON.
   * @param path Chemin relatif au baseDir
   * @param data L'objet à écrire
   */
  async function writeJson(path: string, data: unknown): Promise<void> {
    try {
      const text = JSON.stringify(data, null, 2);
      await writeTextFile(path, text, { baseDir });
    } catch (error) {
      console.error(`[useFileSystem] Erreur écriture ${path}:`, error);
      throw error;
    }
  }

  return {
    readJson,
    writeJson,
  };
}
