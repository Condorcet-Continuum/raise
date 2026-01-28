import { describe, it, expect, beforeEach } from 'vitest';
import { QueryBuilder } from '../../../src/services/json-db/query-builder';

describe('Query Builder', () => {
  let queryBuilder: QueryBuilder;

  beforeEach(() => {
    queryBuilder = new QueryBuilder('test_collection');
  });

  it('builds a simple equality query', () => {
    // CORRECTION : 'Eq' avec une Majuscule
    queryBuilder.where('name', 'Eq', 'Test');

    const query = queryBuilder.build();
    expect(query.filter).toEqual({
      field: 'name',
      operator: 'Eq', // Vérifiez aussi ici
      value: 'Test',
    });
  });

  it('builds a complex query with multiple conditions and sorting', () => {
    queryBuilder
      .where('type', 'Eq', 'service') // 'Eq' Majuscule
      .where('status', 'Ne', 'archived') // 'Ne' Majuscule
      .orderBy('name', 'Asc') // 'Asc' Majuscule
      .limit(10)
      .offset(5);

    const query = queryBuilder.build();

    expect(query.filter).toBeDefined();
    // Note: La structure interne dépend de votre implémentation du builder,
    // assurez-vous que le test reflète la logique 'AND' si vous en avez une.

    expect(query.sort).toEqual({ field: 'name', direction: 'Asc' });
    expect(query.limit).toBe(10);
    expect(query.offset).toBe(5);
  });
});
