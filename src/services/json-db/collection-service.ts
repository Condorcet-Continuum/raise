import { invoke } from '@tauri-apps/api/core';
import { queryService } from './query-service';
import type { Query, Document } from '@/types/json-db.types';

const DEFAULT_SPACE = 'un2';
const DEFAULT_DB = '_system';

export class CollectionService {
  async createCollection(name: string, schemaUri?: string): Promise<void> {
    await invoke('jsondb_create_collection', {
      space: DEFAULT_SPACE,
      db: DEFAULT_DB,
      collection: name,
      schemaUri: schemaUri || null,
    });
  }

  async listAllCollections(): Promise<string[]> {
    return await invoke<string[]>('jsondb_list_collections', {
      space: DEFAULT_SPACE,
      db: DEFAULT_DB,
    });
  }

  async listAll(collection: string): Promise<Document[]> {
    return await invoke<Document[]>('jsondb_list_all', {
      space: DEFAULT_SPACE,
      db: DEFAULT_DB,
      collection,
    });
  }

  async insertDocument(collection: string, doc: any): Promise<any> {
    return await invoke('jsondb_insert_document', {
      space: DEFAULT_SPACE,
      db: DEFAULT_DB,
      collection,
      document: doc,
    });
  }

  async getDocument(collection: string, id: string): Promise<any | null> {
    return await invoke('jsondb_get_document', {
      space: DEFAULT_SPACE,
      db: DEFAULT_DB,
      collection,
      id,
    });
  }

  async updateDocument(collection: string, id: string, doc: any): Promise<any> {
    return await invoke('jsondb_update_document', {
      space: DEFAULT_SPACE,
      db: DEFAULT_DB,
      collection,
      id,
      document: doc,
    });
  }

  async deleteDocument(collection: string, id: string): Promise<boolean> {
    return await invoke('jsondb_delete_document', {
      space: DEFAULT_SPACE,
      db: DEFAULT_DB,
      collection,
      id,
    });
  }

  /**
   * Proxy vers le QueryService.
   * CORRECTION MAJEURE : Signature alignée pour accepter (collection, query, options).
   */
  async queryDocuments(
    collection: string,
    query: Query,
    options?: { latest?: boolean },
  ): Promise<any[]> {
    query.collection = collection;
    return queryService.execute(query, options);
  }
}

export const collectionService = new CollectionService();
