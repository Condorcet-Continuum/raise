/**
 * Service de construction de requêtes
 */

export type FilterOperator = 'and' | 'or' | 'not';
export type ComparisonOperator = 'eq' | 'ne' | 'gt' | 'gte' | 'lt' | 'lte' | 'in' | 'contains' | 'startsWith' | 'endsWith';
export type SortOrder = 'asc' | 'desc';

export interface Condition {
  field: string;
  operator: ComparisonOperator;
  value: any;
}

export interface QueryFilter {
  operator: FilterOperator;
  conditions: Condition[];
}

export interface SortField {
  field: string;
  order: SortOrder;
}

export interface Query {
  collection: string;
  filter?: QueryFilter;
  sort?: SortField[];
  limit?: number;
  offset?: number;
}

export class QueryBuilder {
  private query: Query;

  constructor(collection: string) {
    this.query = { collection };
  }

  where(field: string, operator: ComparisonOperator, value: any): this {
    if (!this.query.filter) {
      this.query.filter = { operator: 'and', conditions: [] };
    }
    this.query.filter.conditions.push({ field, operator, value });
    return this;
  }

  orderBy(field: string, order: SortOrder = 'asc'): this {
    if (!this.query.sort) {
      this.query.sort = [];
    }
    this.query.sort.push({ field, order });
    return this;
  }

  limit(limit: number): this {
    this.query.limit = limit;
    return this;
  }

  offset(offset: number): this {
    this.query.offset = offset;
    return this;
  }

  build(): Query {
    return this.query;
  }
}

export const createQuery = (collection: string) => new QueryBuilder(collection);
