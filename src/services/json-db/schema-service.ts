/**
 * Service de gestion des schémas JSON Schema
 */

//import { invoke } from '@tauri-apps/api/core';

export interface JsonSchema {
  id: string;
  title: string;
  schema_type: string;
  version: string;
  schema: any;
  created_at: number;
}

export class SchemaService {
  // Paramètre inutilisé → préfixe avec _
  async registerSchema(_schema: JsonSchema): Promise<void> {
    // TODO: quand le backend Tauri sera prêt
    // await invoke('register_schema', { schema: _schema });
  }

  // Paramètre inutilisé → préfixe avec _
  async getSchema(_schemaId: string): Promise<JsonSchema> {
    // TODO: quand le backend Tauri sera prêt
    // return await invoke<JsonSchema>('get_schema', { schemaId: _schemaId });
    throw new Error('getSchema not implemented yet');
  }
}

export const schemaService = new SchemaService();
