/**
 * Exemple d'utilisation de JSON-LD
 */

import { jsonLdService, collectionService } from '@/services/json-db';

async function jsonLdExample() {
  // Enregistrer un contexte JSON-LD
  jsonLdService.registerContext('software_component', {
    '@context': {
      '@vocab': 'https://genaptitude.io/vocab/software#',
      'name': 'http://schema.org/name',
      'Component': 'https://genaptitude.io/vocab/software#Component'
    }
  });

  // Document simple
  const component = {
    id: 'comp-001',
    name: 'UserService',
    type: 'Component'
  };

  // Expansion avec contexte sémantique
  const expanded = jsonLdService.expandDocument(component, 'software_component');
  
  console.log('Expanded:', expanded);
}

jsonLdExample().catch(console.error);
