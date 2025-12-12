import { invoke } from '@tauri-apps/api/core';
import { queryService } from './query-service';
import { useSettingsStore } from '@/store/settings-store';
import type { Query, Document } from '@/types/json-db.types';

export class CollectionService {
  /**
   * Récupère la configuration actuelle (Espace et DB) depuis le store global.
   */
  private getConfig() {
    const { jsonDbSpace, jsonDbDatabase } = useSettingsStore.getState();
    return { space: jsonDbSpace, db: jsonDbDatabase };
  }

  // --- DATABASE MANAGEMENT ---

  async createDb(): Promise<void> {
    const { space, db } = this.getConfig();
    await invoke('jsondb_create_db', { space, db });
  }

  async dropDb(): Promise<void> {
    const { space, db } = this.getConfig();
    await invoke('jsondb_drop_db', { space, db });
  }

  // --- COLLECTION MANAGEMENT ---

  async createCollection(name: string, schemaUri?: string): Promise<void> {
    const { space, db } = this.getConfig();
    await invoke('jsondb_create_collection', {
      space,
      db,
      collection: name,
      schemaUri: schemaUri || null,
    });
  }

  async dropCollection(name: string): Promise<void> {
    const { space, db } = this.getConfig();
    await invoke('jsondb_drop_collection', {
      space,
      db,
      collection: name,
    });
  }

  async listAllCollections(): Promise<string[]> {
    const { space, db } = this.getConfig();
    return await invoke<string[]>('jsondb_list_collections', { space, db });
  }

  // --- INDEX MANAGEMENT ---

  async createIndex(
    collection: string,
    field: string,
    kind: 'hash' | 'btree' | 'text' = 'hash',
  ): Promise<void> {
    const { space, db } = this.getConfig();
    await invoke('jsondb_create_index', {
      space,
      db,
      collection,
      field,
      kind,
    });
  }

  async dropIndex(collection: string, field: string): Promise<void> {
    const { space, db } = this.getConfig();
    await invoke('jsondb_drop_index', {
      space,
      db,
      collection,
      field,
    });
  }

  // --- CRUD OPERATIONS ---

  async listAll(collection: string): Promise<Document[]> {
    const { space, db } = this.getConfig();
    return await invoke<Document[]>('jsondb_list_all', {
      space,
      db,
      collection,
    });
  }

  async insertDocument(collection: string, doc: any): Promise<any> {
    const { space, db } = this.getConfig();
    return await invoke('jsondb_insert_document', {
      space,
      db,
      collection,
      document: doc,
    });
  }

  async getDocument(collection: string, id: string): Promise<any | null> {
    const { space, db } = this.getConfig();
    return await invoke('jsondb_get_document', {
      space,
      db,
      collection,
      id,
    });
  }

  async updateDocument(collection: string, id: string, doc: any): Promise<any> {
    const { space, db } = this.getConfig();
    return await invoke('jsondb_update_document', {
      space,
      db,
      collection,
      id,
      document: doc,
    });
  }

  async deleteDocument(collection: string, id: string): Promise<boolean> {
    const { space, db } = this.getConfig();
    return await invoke('jsondb_delete_document', {
      space,
      db,
      collection,
      id,
    });
  }

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
