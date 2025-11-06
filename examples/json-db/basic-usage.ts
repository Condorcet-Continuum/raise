/**
 * Exemple d'utilisation basique de la JSON DB
 */

import { collectionService, createQuery } from '@/services/json-db';

async function basicExample() {
  // 1. Créer une collection
  await collectionService.createCollection(
    'software_components',
    {
      type: 'object',
      required: ['id', 'name'],
      properties: {
        id: { type: 'string' },
        name: { type: 'string' },
        type: { type: 'string' }
      }
    }
  );

  // 2. Insérer des documents
  await collectionService.insertDocument('software_components', {
    id: 'comp-001',
    name: 'UserService',
    type: 'service'
  });

  // 3. Requêter
  const query = createQuery('software_components')
    .where('type', 'eq', 'service')
    .build();

  const results = await collectionService.queryDocuments(
    'software_components',
    query
  );

  console.log('Results:', results);
}

basicExample().catch(console.error);
