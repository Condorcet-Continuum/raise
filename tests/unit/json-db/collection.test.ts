import { describe, it, expect } from 'vitest';
import { collectionService } from '@/services/json-db';

describe('CollectionService', () => {
  it('should create a collection', async () => {
    const result = await collectionService.createCollection(
      'test_collection',
      { type: 'object', properties: { name: { type: 'string' } } }
    );
    
    expect(result).toBeDefined();
    expect(result.name).toBe('test_collection');
  });

  it('should insert a document', async () => {
    const doc = { name: 'Test Document' };
    const result = await collectionService.insertDocument('test_collection', doc);
    
    expect(result).toBeDefined();
    expect(result.id).toBeDefined();
  });
});
