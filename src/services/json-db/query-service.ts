import { invoke } from '@tauri-apps/api/core';
import type { Query, Condition, ComparisonOperator, QueryResponse } from '@/types/json-db.types';

const DEFAULT_SPACE = 'un2';
const DEFAULT_DB = '_system';

export class QueryBuilder {
  private query: Query;

  constructor(collection: string) {
    this.query = {
      collection,
      filter: undefined,
      sort: [],
      limit: undefined,
      offset: undefined,
    };
  }

  where(field: string, op: ComparisonOperator, value: any): this {
    const condition: Condition = { field, operator: op, value };

    if (!this.query.filter) {
      this.query.filter = { operator: 'And', conditions: [condition] };
    } else {
      if (this.query.filter.operator === 'And') {
        this.query.filter.conditions.push(condition);
      } else {
        this.query.filter = { operator: 'And', conditions: [condition] };
      }
    }
    return this;
  }

  orderBy(field: string, order: 'Asc' | 'Desc' = 'Asc'): this {
    if (!this.query.sort) this.query.sort = [];
    this.query.sort.push({ field, order });
    return this;
  }

  limit(n: number): this {
    this.query.limit = n;
    return this;
  }

  offset(n: number): this {
    this.query.offset = n;
    return this;
  }

  build(): Query {
    return this.query;
  }
}

export class JsonDbQueryService {
  // Correction: Accepte explicitement 'options' comme 2ème argument
  async execute(query: Query, options?: { latest?: boolean }): Promise<any[]> {
    try {
      if (options?.latest) {
        if (!query.sort) query.sort = [];
        query.sort.unshift({ field: 'createdAt', order: 'Desc' });
        if (!query.limit) query.limit = 1;
      }

      const res = await invoke<QueryResponse>('jsondb_execute_query', {
        space: DEFAULT_SPACE,
        db: DEFAULT_DB,
        query: query,
      });
      return res.documents;
    } catch (e) {
      console.error('Query Failed:', e);
      throw e;
    }
  }

  async executeSql(sql: string): Promise<any[]> {
    try {
      const res = await invoke<QueryResponse>('jsondb_execute_sql', {
        space: DEFAULT_SPACE,
        db: DEFAULT_DB,
        sql: sql,
      });
      return res.documents;
    } catch (e) {
      console.error('SQL Failed:', e);
      throw e;
    }
  }
}

export const queryService = new JsonDbQueryService();
export const createQuery = (collection: string) => new QueryBuilder(collection);
