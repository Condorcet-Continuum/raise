import { describe, it, expect } from 'vitest';
import { createQuery } from '@/services/json-db';

describe('QueryBuilder', () => {
  it('should build a simple query', () => {
    const query = createQuery('test_collection')
      .where('name', 'eq', 'Test')
      .build();
    
    expect(query.collection).toBe('test_collection');
    expect(query.filter?.conditions).toHaveLength(1);
  });

  it('should build a complex query', () => {
    const query = createQuery('test_collection')
      .where('type', 'eq', 'service')
      .where('status', 'ne', 'archived')
      .orderBy('name', 'asc')
      .limit(10)
      .build();
    
    expect(query.filter?.conditions).toHaveLength(2);
    expect(query.sort).toHaveLength(1);
    expect(query.limit).toBe(10);
  });
});
