// src/services/json-db/schema-service.ts

// Note: Actuellement, la gestion des schémas (fichiers physiques) se fait côté Backend/FS.
// Ce service sert d'utilitaire pour manipuler les références.

export type JsonSchema = Record<string, any>;

export class SchemaService {
  /**
   * Construit une URI de schéma valide pour le backend Rust.
   * Format: db://<space>/<db>/schemas/v1/<path>
   */
  getSchemaUri(space: string, db: string, relativePath: string): string {
    // Nettoyage du path pour éviter les doubles slashes
    const cleanPath = relativePath.startsWith('/') ? relativePath.slice(1) : relativePath;
    return `db://${space}/${db}/schemas/v1/${cleanPath}`;
  }

  /**
   * Enregistre un schéma.
   * ⚠️ NON IMPLÉMENTÉ CÔTÉ BACKEND IPC POUR L'INSTANT.
   */
  async registerSchema(schemaId: string, schema: JsonSchema): Promise<void> {
    // CORRECTION : On utilise les variables dans le log pour éviter l'erreur "unused variable"
    console.warn(`SchemaService: registerSchema [${schemaId}] via IPC not supported.`, schema);
    console.log("Pour l'instant, déposez le fichier JSON dans le dossier de la DB sur le disque.");
    return Promise.resolve();
  }

  /**
   * Récupère un schéma.
   * ⚠️ NON IMPLÉMENTÉ CÔTÉ BACKEND IPC POUR L'INSTANT.
   */
  async getSchema(schemaId: string): Promise<JsonSchema | null> {
    // CORRECTION : On utilise la variable dans le log
    console.warn(`SchemaService: getSchema [${schemaId}] via IPC not supported.`);
    return Promise.resolve(null);
  }
}

export const schemaService = new SchemaService();
