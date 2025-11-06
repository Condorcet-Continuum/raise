/**
 * Types pour la base de données JSON
 */

export interface Collection {
  name: string;
  schema_id: string;
  jsonld_context?: string;
  indexes: string[];
  created_at: number;
  updated_at: number;
}

export interface Document<T = any> {
  id: string;
  collection: string;
  data: T;
  version: number;
  created_at: number;
  updated_at: number;
}

export interface JsonSchema {
  id: string;
  title: string;
  schema_type: string;
  version: string;
  schema: Record<string, any>;
  created_at: number;
}

export interface Index {
  name: string;
  collection: string;
  fields: string[];
  index_type: 'btree' | 'hash' | 'text';
  unique: boolean;
}

export interface Migration {
  id: string;
  version: string;
  description: string;
  up: MigrationStep[];
  down: MigrationStep[];
  applied_at?: number;
}

export type MigrationStep =
  | { type: 'create_collection'; name: string; schema: any }
  | { type: 'drop_collection'; name: string }
  | { type: 'add_field'; collection: string; field: string; default?: any }
  | { type: 'remove_field'; collection: string; field: string }
  | { type: 'rename_field'; collection: string; old_name: string; new_name: string }
  | { type: 'create_index'; collection: string; fields: string[] }
  | { type: 'drop_index'; collection: string; name: string };

export interface Transaction {
  id: string;
  operations: TransactionOperation[];
  status: 'pending' | 'committed' | 'aborted';
  started_at: number;
}

export type TransactionOperation =
  | { type: 'insert'; collection: string; document: any }
  | { type: 'update'; collection: string; id: string; document: any }
  | { type: 'delete'; collection: string; id: string };
