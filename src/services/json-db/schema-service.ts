// src/services/json-db/schema-service.ts

export type JsonSchema = Record<string, any>;

export class SchemaService {
  /**
   * Construit une URI de schéma valide pour le backend Rust.
   * Format: db://<space>/<db>/schemas/v1/<path>
   */
  getSchemaUri(space: string, db: string, relativePath: string): string {
    const cleanPath = relativePath.startsWith('/') ? relativePath.slice(1) : relativePath;
    return `db://${space}/${db}/schemas/v1/${cleanPath}`;
  }

  /**
   * Enregistre un schéma.
   * ⚠️ Placeholder : Le backend gère les fichiers physiques pour le moment.
   */
  async registerSchema(schemaId: string, schema: JsonSchema): Promise<void> {
    console.warn(
      `[SchemaService] registerSchema '${schemaId}' skipped (Backend handles FS).`,
      schema,
    );
    return Promise.resolve();
  }

  /**
   * Récupère un schéma.
   * ⚠️ Placeholder.
   */
  async getSchema(schemaId: string): Promise<JsonSchema | null> {
    console.warn(`[SchemaService] getSchema '${schemaId}' skipped.`);
    return Promise.resolve(null);
  }
}

export const schemaService = new SchemaService();
