# Base de Données JSON

## Vue d'ensemble

GenAptitude utilise une base de données JSON embarquée avec support de:
- **Collections** avec schémas JSON Schema
- **Contextes JSON-LD** pour données liées sémantiques
- **Indexes** pour requêtes rapides
- **Transactions** ACID
- **Migrations** de schémas

## Architecture

```
src-tauri/src/json_db/
├── collections/      # Gestion des collections
├── schema/          # Validation JSON Schema
├── jsonld/          # Support JSON-LD
├── query/           # Moteur de requêtes
├── storage/         # Stockage sur disque
├── transactions/    # Gestion ACID
├── indexes/         # Système d'indexation
└── migrations/      # Migrations de schémas
```

## Utilisation Frontend

### Créer une collection

```typescript
import { collectionService } from '@/services/json-db';
import componentSchema from '@/domain-models/software/json-schemas/component.schema.json';

await collectionService.createCollection(
  'software_components',
  componentSchema
);
```

### Insérer un document

```typescript
const component = {
  id: 'comp-001',
  name: 'UserService',
  type: 'service',
  interfaces: [
    { name: 'HTTP', type: 'input', protocol: 'REST' }
  ]
};

await collectionService.insertDocument('software_components', component);
```

### Requêtes

```typescript
import { createQuery } from '@/services/json-db';

const query = createQuery('software_components')
  .where('type', 'eq', 'service')
  .where('name', 'contains', 'Service')
  .orderBy('name', 'asc')
  .limit(10)
  .build();

const results = await collectionService.queryDocuments(
  'software_components',
  query
);
```

### JSON-LD

```typescript
import { jsonLdService } from '@/services/json-db';
import context from '@/domain-models/software/jsonld-contexts/component.context.json';

jsonLdService.registerContext('software_component', context);

const expanded = jsonLdService.expandDocument(component, 'software_component');
```

## Schémas par Domaine

### Software Engineering
- `domain-models/software/json-schemas/` - Schémas JSON Schema
- `domain-models/software/jsonld-contexts/` - Contextes JSON-LD

### System Engineering
- `domain-models/system/json-schemas/` - Schémas JSON Schema
- `domain-models/system/jsonld-contexts/` - Contextes JSON-LD

### Hardware Engineering
- `domain-models/hardware/json-schemas/` - Schémas JSON Schema
- `domain-models/hardware/jsonld-contexts/` - Contextes JSON-LD

## Migrations

Les migrations permettent de faire évoluer les schémas:

```rust
Migration {
    id: "001_create_components",
    version: "1.0.0",
    description: "Create software components collection",
    up: vec![
        MigrationStep::CreateCollection {
            name: "software_components".to_string(),
            schema: component_schema,
        },
    ],
    down: vec![
        MigrationStep::DropCollection {
            name: "software_components".to_string(),
        },
    ],
    applied_at: None,
}
```

## Performance

- **Indexes**: Créez des indexes sur les champs fréquemment requis
- **Cache**: Les documents sont mis en cache automatiquement
- **Compression**: Activée par défaut pour économiser l'espace disque

## Transactions

```typescript
// TODO: API de transactions depuis le frontend
```

## Ressources

- [JSON Schema](https://json-schema.org/)
- [JSON-LD](https://json-ld.org/)
- [Linked Data](https://www.w3.org/standards/semanticweb/data)
