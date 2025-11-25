// src/services/json-db/collection-service.ts

import { invoke } from '@tauri-apps/api/core';

const DEFAULT_SPACE = 'un2';
const DEFAULT_DB = '_system';

export interface Document<T = any> {
  id: string;
  [key: string]: T | any;
}

export class CollectionService {
  /**
   * Crée une collection avec un schéma spécifique.
   * @param name Nom de la collection
   * @param schema Chemin relatif du schéma (ex: "sandbox/generic.schema.json")
   */
  async createCollection(name: string, schema?: string): Promise<void> {
    await invoke('jsondb_create_collection', {
      space: DEFAULT_SPACE,
      db: DEFAULT_DB,
      collection: name,
      schema: schema ?? 'sandbox/generic.schema.json',
    });
  }

  async insertRaw(collection: string, doc: any): Promise<void> {
    await invoke('jsondb_insert_raw', {
      space: DEFAULT_SPACE,
      db: DEFAULT_DB,
      collection,
      doc,
    });
  }

  async listAll(collection: string): Promise<Document[]> {
    return await invoke<Document[]>('jsondb_list_all', {
      space: DEFAULT_SPACE,
      db: DEFAULT_DB,
      collection,
    });
  }
}

export const collectionService = new CollectionService();
