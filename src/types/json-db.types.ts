// FICHIER : src/types/json-db.types.ts

// --- Query Engine ---

export type SortOrder = 'Asc' | 'Desc';

export interface SortField {
  field: string;
  order: SortOrder;
}

export type FilterOperator = 'And' | 'Or' | 'Not';

export type ComparisonOperator =
  | 'Eq'
  | 'Ne'
  | 'Gt'
  | 'Gte'
  | 'Lt'
  | 'Lte'
  | 'In'
  | 'Contains'
  | 'StartsWith'
  | 'EndsWith'
  | 'Matches';

export interface Condition {
  field: string;
  operator: ComparisonOperator;
  value: any;
}

export interface QueryFilter {
  operator: FilterOperator;
  conditions: Condition[];
}

export interface Query {
  collection: string;
  filter?: QueryFilter;
  sort?: SortField[];
  limit?: number;
  offset?: number;
  projection?: string[];
}

export interface QueryResponse {
  documents: any[];
  total: number;
}

// --- Transactions ---

export type OperationRequest =
  | { type: 'Insert'; collection: string; id: string; document: any }
  | { type: 'Update'; collection: string; id: string; document: any }
  | { type: 'Delete'; collection: string; id: string };

export interface TransactionRequest {
  operations: OperationRequest[];
}

// --- Document Générique ---

export interface Document<T = any> {
  id: string;
  [key: string]: T | any;
}
