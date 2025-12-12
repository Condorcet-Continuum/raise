import { invoke } from '@tauri-apps/api/core';
import { useSettingsStore } from '@/store/settings-store';
import type { Query, Condition, ComparisonOperator, QueryResponse } from '@/types/json-db.types';

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
  private getConfig() {
    const { jsonDbSpace, jsonDbDatabase } = useSettingsStore.getState();
    return { space: jsonDbSpace, db: jsonDbDatabase };
  }

  async execute(query: Query, options?: { latest?: boolean }): Promise<any[]> {
    try {
      const { space, db } = this.getConfig();

      // Application des options rapides (ex: récupérer le dernier élément)
      if (options?.latest) {
        if (!query.sort) query.sort = [];
        query.sort.unshift({ field: 'createdAt', order: 'Desc' });
        if (!query.limit) query.limit = 1;
      }

      const res = await invoke<QueryResponse>('jsondb_execute_query', {
        space,
        db,
        query,
      });
      return res.documents;
    } catch (e) {
      console.error('[QueryService] Execute Failed:', e);
      throw e;
    }
  }

  async executeSql(sql: string): Promise<any[]> {
    try {
      const { space, db } = this.getConfig();
      const res = await invoke<QueryResponse>('jsondb_execute_sql', {
        space,
        db,
        sql,
      });
      return res.documents;
    } catch (e) {
      console.error('[QueryService] SQL Failed:', e);
      throw e;
    }
  }
}

export const queryService = new JsonDbQueryService();
export const createQuery = (collection: string) => new QueryBuilder(collection);
