/**
 * Service de gestion des collections
 */

import { invoke } from '@tauri-apps/api/tauri';

export interface Collection {
  name: string;
  schema_id: string;
  jsonld_context?: string;
  indexes: string[];
  created_at: number;
  updated_at: number;
}

export interface Document {
  id: string;
  collection: string;
  data: any;
  version: number;
  created_at: number;
  updated_at: number;
}

export class CollectionService {
  async createCollection(
    name: string,
    schema: any,
    context?: any
  ): Promise<Collection> {
    return await invoke('create_collection', { name, schema, context });
  }

  async insertDocument(collection: string, document: any): Promise<Document> {
    return await invoke('insert_document', { collection, document });
  }

  async queryDocuments(collection: string, query: any): Promise<Document[]> {
    return await invoke('query_documents', { collection, query });
  }

  async updateDocument(
    collection: string,
    id: string,
    document: any
  ): Promise<Document> {
    return await invoke('update_document', { collection, id, document });
  }

  async deleteDocument(collection: string, id: string): Promise<void> {
    await invoke('delete_document', { collection, id });
  }

  async getDocument(collection: string, id: string): Promise<Document> {
    const results = await this.queryDocuments(collection, {
      filter: { operator: 'and', conditions: [{ field: 'id', operator: 'eq', value: id }] }
    });
    return results[0];
  }
}

export const collectionService = new CollectionService();
