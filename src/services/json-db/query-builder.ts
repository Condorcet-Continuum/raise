export type ComparisonOperator = 'Eq' | 'Ne' | 'Gt' | 'Lt' | 'Ge' | 'Le' | 'Contains';
export type SortDirection = 'Asc' | 'Desc';

// 1. Définition d'un type union pour éviter 'any'
export type QueryValue = string | number | boolean | null | undefined;

export interface FilterCondition {
  field: string;
  operator: ComparisonOperator;
  value: QueryValue;
}

export interface Query {
  collection: string;
  filter?: FilterCondition;
  sort?: { field: string; direction: SortDirection };
  limit?: number;
  offset?: number;
}

export class QueryBuilder {
  private collection: string;
  private query: Query;

  constructor(collectionName: string) {
    this.collection = collectionName;
    this.query = { collection: collectionName };
  }

  // 2. Utilisation du type QueryValue
  where(field: string, operator: ComparisonOperator, value: QueryValue): QueryBuilder {
    this.query.filter = { field, operator, value };
    return this;
  }

  orderBy(field: string, direction: SortDirection = 'Asc'): QueryBuilder {
    this.query.sort = { field, direction };
    return this;
  }

  limit(count: number): QueryBuilder {
    this.query.limit = count;
    return this;
  }

  offset(count: number): QueryBuilder {
    this.query.offset = count;
    return this;
  }

  build(): Query {
    return this.query;
  }
}
