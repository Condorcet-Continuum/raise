import { invoke } from '@tauri-apps/api/core';

const DEFAULT_SPACE = 'un2';
const DEFAULT_DB = '_system';

// On utilise T pour typer le champ "data" si besoin, ou les champs racines
export interface Document<T = any> {
  id: string;
  // On permet d'accéder aux propriétés typées par T
  [key: string]: T | any; 
}

export class CollectionService {
  
  async createCollection(name: string): Promise<void> {
    await invoke('jsondb_create_collection', { 
      space: DEFAULT_SPACE, 
      db: DEFAULT_DB, 
      collection: name 
    });
  }

  async insertRaw(collection: string, doc: any): Promise<void> {
    await invoke('jsondb_insert_raw', { 
      space: DEFAULT_SPACE, 
      db: DEFAULT_DB, 
      collection, 
      doc 
    });
  }

  async listAll(collection: string): Promise<Document[]> {
    return await invoke<Document[]>('jsondb_list_all', { 
      space: DEFAULT_SPACE, 
      db: DEFAULT_DB, 
      collection 
    });
  }
}

export const collectionService = new CollectionService();