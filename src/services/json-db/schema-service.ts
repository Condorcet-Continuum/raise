/**
 * Service de gestion des schémas JSON Schema
 */

import { invoke } from '@tauri-apps/api/tauri';

export interface JsonSchema {
  id: string;
  title: string;
  schema_type: string;
  version: string;
  schema: any;
  created_at: number;
}

export class SchemaService {
  async validateDocument(collection: string, document: any): Promise<boolean> {
    return await invoke('validate_document', { collection, document });
  }

  async registerSchema(schema: JsonSchema): Promise<void> {
    // TODO: Implémenter
  }

  async getSchema(schemaId: string): Promise<JsonSchema> {
    // TODO: Implémenter
    throw new Error('Not implemented');
  }
}

export const schemaService = new SchemaService();
