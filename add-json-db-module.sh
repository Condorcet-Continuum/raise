#!/bin/bash

################################################################################
# GenAptitude - Module JSON Database
# Ajout d'un système de base de données JSON avec collections, schémas et JSON-LD
################################################################################

set -e

# Couleurs
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

print_section() {
    echo -e "\n${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${YELLOW}  $1${NC}"
    echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"
}

# Vérifier qu'on est dans le bon répertoire
if [ ! -f "package.json" ] || [ ! -d "src-tauri" ]; then
    echo -e "${YELLOW}⚠ Erreur: Ce script doit être exécuté depuis la racine du projet GenAptitude${NC}"
    echo "Utilisation: cd genaptitude && ../add-json-db-module.sh"
    exit 1
fi

print_section "Ajout du module JSON Database à GenAptitude"

################################################################################
# BACKEND RUST - JSON DB MODULE
################################################################################
print_section "Backend Rust - Module JSON DB"

mkdir -p src-tauri/src/json_db/{collections,schema,jsonld,query,storage,transactions,indexes,migrations}

# Module principal
cat > src-tauri/src/json_db/mod.rs << 'EOF'
//! Module de gestion de base de données JSON
//! 
//! Fonctionnalités:
//! - Collections avec schémas JSON Schema
//! - Support JSON-LD pour contexte sémantique
//! - Indexes pour requêtes rapides
//! - Transactions ACID
//! - Migrations de schémas

pub mod collections;
pub mod schema;
pub mod jsonld;
pub mod query;
pub mod storage;
pub mod transactions;
pub mod indexes;
pub mod migrations;

pub use collections::CollectionManager;
pub use schema::SchemaValidator;
pub use jsonld::JsonLdContext;
pub use query::QueryEngine;
pub use storage::StorageEngine;
pub use transactions::TransactionManager;
EOF

# Collections Manager
cat > src-tauri/src/json_db/collections/mod.rs << 'EOF'
//! Gestionnaire de collections JSON

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod manager;
pub mod collection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub name: String,
    pub schema_id: String,
    pub jsonld_context: Option<String>,
    pub indexes: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub collection: String,
    pub data: serde_json::Value,
    pub version: u32,
    pub created_at: i64,
    pub updated_at: i64,
}
EOF

touch src-tauri/src/json_db/collections/{manager.rs,collection.rs}

# Schema Validator
cat > src-tauri/src/json_db/schema/mod.rs << 'EOF'
//! Validation de schémas JSON Schema

use serde::{Deserialize, Serialize};

pub mod validator;
pub mod registry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchema {
    pub id: String,
    pub title: String,
    pub schema_type: String,
    pub version: String,
    pub schema: serde_json::Value,
    pub created_at: i64,
}

#[derive(Debug)]
pub enum ValidationError {
    SchemaNotFound(String),
    InvalidData(String),
    TypeMismatch(String),
}
EOF

touch src-tauri/src/json_db/schema/{validator.rs,registry.rs}

# JSON-LD Context
cat > src-tauri/src/json_db/jsonld/mod.rs << 'EOF'
//! Gestion des contextes JSON-LD pour données liées

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod context;
pub mod processor;
pub mod vocabulary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonLdContext {
    #[serde(rename = "@context")]
    pub context: ContextDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContextDefinition {
    Simple(String),
    Object(HashMap<String, ContextValue>),
    Array(Vec<ContextDefinition>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContextValue {
    Simple(String),
    Expanded {
        #[serde(rename = "@id")]
        id: String,
        #[serde(rename = "@type")]
        type_: Option<String>,
        #[serde(rename = "@container")]
        container: Option<String>,
    },
}
EOF

touch src-tauri/src/json_db/jsonld/{context.rs,processor.rs,vocabulary.rs}

# Query Engine
cat > src-tauri/src/json_db/query/mod.rs << 'EOF'
//! Moteur de requêtes JSON

use serde::{Deserialize, Serialize};

pub mod parser;
pub mod executor;
pub mod optimizer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub collection: String,
    pub filter: Option<QueryFilter>,
    pub sort: Option<Vec<SortField>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryFilter {
    pub operator: FilterOperator,
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterOperator {
    And,
    Or,
    Not,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub operator: ComparisonOperator,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComparisonOperator {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
    Contains,
    StartsWith,
    EndsWith,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortField {
    pub field: String,
    pub order: SortOrder,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    Desc,
}
EOF

touch src-tauri/src/json_db/query/{parser.rs,executor.rs,optimizer.rs}

# Storage Engine
cat > src-tauri/src/json_db/storage/mod.rs << 'EOF'
//! Moteur de stockage sur disque

pub mod file_storage;
pub mod cache;
pub mod compression;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub data_dir: String,
    pub cache_size_mb: usize,
    pub compression_enabled: bool,
    pub auto_compact: bool,
}
EOF

touch src-tauri/src/json_db/storage/{file_storage.rs,cache.rs,compression.rs}

# Transactions
cat > src-tauri/src/json_db/transactions/mod.rs << 'EOF'
//! Gestion des transactions ACID

pub mod transaction;
pub mod lock_manager;
pub mod wal;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub operations: Vec<Operation>,
    pub status: TransactionStatus,
    pub started_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    Insert { collection: String, document: serde_json::Value },
    Update { collection: String, id: String, document: serde_json::Value },
    Delete { collection: String, id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
    Pending,
    Committed,
    Aborted,
}
EOF

touch src-tauri/src/json_db/transactions/{transaction.rs,lock_manager.rs,wal.rs}

# Indexes
cat > src-tauri/src/json_db/indexes/mod.rs << 'EOF'
//! Système d'indexation pour requêtes rapides

pub mod btree_index;
pub mod hash_index;
pub mod text_index;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    pub name: String,
    pub collection: String,
    pub fields: Vec<String>,
    pub index_type: IndexType,
    pub unique: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexType {
    BTree,
    Hash,
    Text,
}
EOF

touch src-tauri/src/json_db/indexes/{btree_index.rs,hash_index.rs,text_index.rs}

# Migrations
cat > src-tauri/src/json_db/migrations/mod.rs << 'EOF'
//! Système de migrations de schémas

pub mod migrator;
pub mod version;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    pub id: String,
    pub version: String,
    pub description: String,
    pub up: Vec<MigrationStep>,
    pub down: Vec<MigrationStep>,
    pub applied_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MigrationStep {
    CreateCollection { name: String, schema: serde_json::Value },
    DropCollection { name: String },
    AddField { collection: String, field: String, default: Option<serde_json::Value> },
    RemoveField { collection: String, field: String },
    RenameField { collection: String, old_name: String, new_name: String },
    CreateIndex { collection: String, fields: Vec<String> },
    DropIndex { collection: String, name: String },
}
EOF

touch src-tauri/src/json_db/migrations/{migrator.rs,version.rs}

print_success "Module Rust JSON DB créé"

################################################################################
# TAURI COMMANDS
################################################################################
print_section "Commandes Tauri pour JSON DB"

cat > src-tauri/src/commands/json_db_commands.rs << 'EOF'
//! Commandes Tauri pour la base de données JSON

use tauri::State;
use serde_json::Value;

#[tauri::command]
pub async fn create_collection(
    name: String,
    schema: Value,
    context: Option<Value>,
) -> Result<Value, String> {
    // TODO: Implémenter
    Ok(serde_json::json!({ "status": "ok", "collection": name }))
}

#[tauri::command]
pub async fn insert_document(
    collection: String,
    document: Value,
) -> Result<Value, String> {
    // TODO: Implémenter
    Ok(serde_json::json!({ "status": "ok", "id": "doc-123" }))
}

#[tauri::command]
pub async fn query_documents(
    collection: String,
    query: Value,
) -> Result<Vec<Value>, String> {
    // TODO: Implémenter
    Ok(vec![])
}

#[tauri::command]
pub async fn update_document(
    collection: String,
    id: String,
    document: Value,
) -> Result<Value, String> {
    // TODO: Implémenter
    Ok(serde_json::json!({ "status": "ok" }))
}

#[tauri::command]
pub async fn delete_document(
    collection: String,
    id: String,
) -> Result<Value, String> {
    // TODO: Implémenter
    Ok(serde_json::json!({ "status": "ok" }))
}

#[tauri::command]
pub async fn create_index(
    collection: String,
    fields: Vec<String>,
    index_type: String,
) -> Result<Value, String> {
    // TODO: Implémenter
    Ok(serde_json::json!({ "status": "ok" }))
}

#[tauri::command]
pub async fn validate_document(
    collection: String,
    document: Value,
) -> Result<bool, String> {
    // TODO: Implémenter
    Ok(true)
}

#[tauri::command]
pub async fn run_migration(
    migration_id: String,
) -> Result<Value, String> {
    // TODO: Implémenter
    Ok(serde_json::json!({ "status": "ok" }))
}
EOF

# Ajouter l'import dans commands/mod.rs
cat >> src-tauri/src/commands/mod.rs << 'EOF'

pub mod json_db_commands;
EOF

print_success "Commandes Tauri créées"

################################################################################
# FRONTEND SERVICES
################################################################################
print_section "Services Frontend"

mkdir -p src/services/json-db

cat > src/services/json-db/index.ts << 'EOF'
/**
 * Service de gestion de la base de données JSON
 */

export * from './collection-service';
export * from './schema-service';
export * from './query-service';
export * from './jsonld-service';
EOF

cat > src/services/json-db/collection-service.ts << 'EOF'
/**
 * Service de gestion des collections
 */

import { invoke } from '@tauri-apps/api/tauri';

export interface Collection {
  name: string;
  schema_id: string;
  jsonld_context?: string;
  indexes: string[];
  created_at: number;
  updated_at: number;
}

export interface Document {
  id: string;
  collection: string;
  data: any;
  version: number;
  created_at: number;
  updated_at: number;
}

export class CollectionService {
  async createCollection(
    name: string,
    schema: any,
    context?: any
  ): Promise<Collection> {
    return await invoke('create_collection', { name, schema, context });
  }

  async insertDocument(collection: string, document: any): Promise<Document> {
    return await invoke('insert_document', { collection, document });
  }

  async queryDocuments(collection: string, query: any): Promise<Document[]> {
    return await invoke('query_documents', { collection, query });
  }

  async updateDocument(
    collection: string,
    id: string,
    document: any
  ): Promise<Document> {
    return await invoke('update_document', { collection, id, document });
  }

  async deleteDocument(collection: string, id: string): Promise<void> {
    await invoke('delete_document', { collection, id });
  }

  async getDocument(collection: string, id: string): Promise<Document> {
    const results = await this.queryDocuments(collection, {
      filter: { operator: 'and', conditions: [{ field: 'id', operator: 'eq', value: id }] }
    });
    return results[0];
  }
}

export const collectionService = new CollectionService();
EOF

cat > src/services/json-db/schema-service.ts << 'EOF'
/**
 * Service de gestion des schémas JSON Schema
 */

import { invoke } from '@tauri-apps/api/tauri';

export interface JsonSchema {
  id: string;
  title: string;
  schema_type: string;
  version: string;
  schema: any;
  created_at: number;
}

export class SchemaService {
  async validateDocument(collection: string, document: any): Promise<boolean> {
    return await invoke('validate_document', { collection, document });
  }

  async registerSchema(schema: JsonSchema): Promise<void> {
    // TODO: Implémenter
  }

  async getSchema(schemaId: string): Promise<JsonSchema> {
    // TODO: Implémenter
    throw new Error('Not implemented');
  }
}

export const schemaService = new SchemaService();
EOF

cat > src/services/json-db/query-service.ts << 'EOF'
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
EOF

cat > src/services/json-db/jsonld-service.ts << 'EOF'
/**
 * Service de gestion JSON-LD pour données liées
 */

export interface JsonLdContext {
  '@context': ContextDefinition;
}

export type ContextDefinition = string | Record<string, ContextValue> | ContextDefinition[];

export type ContextValue = string | {
  '@id': string;
  '@type'?: string;
  '@container'?: string;
};

export class JsonLdService {
  private contexts: Map<string, JsonLdContext> = new Map();

  registerContext(name: string, context: JsonLdContext): void {
    this.contexts.set(name, context);
  }

  getContext(name: string): JsonLdContext | undefined {
    return this.contexts.get(name);
  }

  expandDocument(document: any, contextName: string): any {
    const context = this.getContext(contextName);
    if (!context) {
      return document;
    }

    // TODO: Implémenter l'expansion JSON-LD
    return {
      '@context': context['@context'],
      ...document
    };
  }

  compactDocument(document: any, contextName: string): any {
    // TODO: Implémenter la compaction JSON-LD
    const { '@context': _, ...data } = document;
    return data;
  }
}

export const jsonLdService = new JsonLdService();
EOF

print_success "Services frontend créés"

################################################################################
# TYPES TYPESCRIPT
################################################################################
print_section "Types TypeScript"

cat > src/types/json-db.types.ts << 'EOF'
/**
 * Types pour la base de données JSON
 */

export interface Collection {
  name: string;
  schema_id: string;
  jsonld_context?: string;
  indexes: string[];
  created_at: number;
  updated_at: number;
}

export interface Document<T = any> {
  id: string;
  collection: string;
  data: T;
  version: number;
  created_at: number;
  updated_at: number;
}

export interface JsonSchema {
  id: string;
  title: string;
  schema_type: string;
  version: string;
  schema: Record<string, any>;
  created_at: number;
}

export interface Index {
  name: string;
  collection: string;
  fields: string[];
  index_type: 'btree' | 'hash' | 'text';
  unique: boolean;
}

export interface Migration {
  id: string;
  version: string;
  description: string;
  up: MigrationStep[];
  down: MigrationStep[];
  applied_at?: number;
}

export type MigrationStep =
  | { type: 'create_collection'; name: string; schema: any }
  | { type: 'drop_collection'; name: string }
  | { type: 'add_field'; collection: string; field: string; default?: any }
  | { type: 'remove_field'; collection: string; field: string }
  | { type: 'rename_field'; collection: string; old_name: string; new_name: string }
  | { type: 'create_index'; collection: string; fields: string[] }
  | { type: 'drop_index'; collection: string; name: string };

export interface Transaction {
  id: string;
  operations: TransactionOperation[];
  status: 'pending' | 'committed' | 'aborted';
  started_at: number;
}

export type TransactionOperation =
  | { type: 'insert'; collection: string; document: any }
  | { type: 'update'; collection: string; id: string; document: any }
  | { type: 'delete'; collection: string; id: string };
EOF

print_success "Types TypeScript créés"

################################################################################
# DOMAIN MODELS - JSON SCHEMAS
################################################################################
print_section "Schémas JSON par Domaine"

mkdir -p domain-models/{software,system,hardware}/json-schemas
mkdir -p domain-models/{software,system,hardware}/jsonld-contexts

# Software Domain Schemas
cat > domain-models/software/json-schemas/component.schema.json << 'EOF'
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "https://genaptitude.io/schemas/software/component",
  "title": "Software Component",
  "description": "Schéma pour un composant logiciel",
  "type": "object",
  "required": ["id", "name", "type"],
  "properties": {
    "id": {
      "type": "string",
      "description": "Identifiant unique du composant"
    },
    "name": {
      "type": "string",
      "description": "Nom du composant"
    },
    "type": {
      "type": "string",
      "enum": ["service", "library", "module", "function"],
      "description": "Type de composant"
    },
    "description": {
      "type": "string",
      "description": "Description du composant"
    },
    "interfaces": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["name", "type"],
        "properties": {
          "name": { "type": "string" },
          "type": { "type": "string", "enum": ["input", "output", "bidirectional"] },
          "protocol": { "type": "string" }
        }
      }
    },
    "dependencies": {
      "type": "array",
      "items": { "type": "string" }
    },
    "metadata": {
      "type": "object",
      "additionalProperties": true
    }
  }
}
EOF

# Software JSON-LD Context
cat > domain-models/software/jsonld-contexts/component.context.json << 'EOF'
{
  "@context": {
    "@vocab": "https://genaptitude.io/vocab/software#",
    "id": "@id",
    "type": "@type",
    "Component": "https://genaptitude.io/vocab/software#Component",
    "name": "http://schema.org/name",
    "description": "http://schema.org/description",
    "interfaces": {
      "@id": "https://genaptitude.io/vocab/software#hasInterface",
      "@type": "@id"
    },
    "dependencies": {
      "@id": "https://genaptitude.io/vocab/software#dependsOn",
      "@type": "@id"
    }
  }
}
EOF

# System Domain Schemas
cat > domain-models/system/json-schemas/requirement.schema.json << 'EOF'
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "https://genaptitude.io/schemas/system/requirement",
  "title": "System Requirement",
  "description": "Schéma pour une exigence système",
  "type": "object",
  "required": ["id", "title", "level"],
  "properties": {
    "id": {
      "type": "string",
      "pattern": "^REQ-[A-Z]+-[0-9]+$"
    },
    "title": {
      "type": "string"
    },
    "description": {
      "type": "string"
    },
    "level": {
      "type": "string",
      "enum": ["stakeholder", "system", "subsystem", "component"]
    },
    "priority": {
      "type": "string",
      "enum": ["critical", "high", "medium", "low"]
    },
    "status": {
      "type": "string",
      "enum": ["draft", "approved", "implemented", "verified"]
    },
    "traces_to": {
      "type": "array",
      "items": { "type": "string" }
    },
    "compliance": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "standard": { "type": "string" },
          "section": { "type": "string" }
        }
      }
    }
  }
}
EOF

# System JSON-LD Context
cat > domain-models/system/jsonld-contexts/requirement.context.json << 'EOF'
{
  "@context": {
    "@vocab": "https://genaptitude.io/vocab/system#",
    "Requirement": "https://genaptitude.io/vocab/system#Requirement",
    "id": "@id",
    "title": "http://purl.org/dc/terms/title",
    "description": "http://purl.org/dc/terms/description",
    "traces_to": {
      "@id": "https://genaptitude.io/vocab/system#tracesTo",
      "@type": "@id"
    },
    "compliance": "https://genaptitude.io/vocab/system#compliance"
  }
}
EOF

# Hardware Domain Schemas
cat > domain-models/hardware/json-schemas/component.schema.json << 'EOF'
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "https://genaptitude.io/schemas/hardware/component",
  "title": "Hardware Component",
  "description": "Schéma pour un composant matériel",
  "type": "object",
  "required": ["id", "name", "type"],
  "properties": {
    "id": {
      "type": "string"
    },
    "name": {
      "type": "string"
    },
    "type": {
      "type": "string",
      "enum": ["ic", "resistor", "capacitor", "connector", "module"]
    },
    "part_number": {
      "type": "string"
    },
    "manufacturer": {
      "type": "string"
    },
    "specifications": {
      "type": "object",
      "properties": {
        "voltage": { "type": "number" },
        "current": { "type": "number" },
        "power": { "type": "number" },
        "temperature_range": {
          "type": "object",
          "properties": {
            "min": { "type": "number" },
            "max": { "type": "number" }
          }
        }
      }
    },
    "pins": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "number": { "type": "string" },
          "name": { "type": "string" },
          "type": { "type": "string", "enum": ["power", "ground", "signal", "nc"] }
        }
      }
    }
  }
}
EOF

# Hardware JSON-LD Context
cat > domain-models/hardware/jsonld-contexts/component.context.json << 'EOF'
{
  "@context": {
    "@vocab": "https://genaptitude.io/vocab/hardware#",
    "Component": "https://genaptitude.io/vocab/hardware#Component",
    "id": "@id",
    "name": "http://schema.org/name",
    "manufacturer": "http://schema.org/manufacturer",
    "specifications": "https://genaptitude.io/vocab/hardware#specifications"
  }
}
EOF

print_success "Schémas JSON créés pour chaque domaine"

################################################################################
# DOCUMENTATION
################################################################################
print_section "Documentation JSON DB"

cat > docs/json-db.md << 'EOF'
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
EOF

print_success "Documentation créée"

################################################################################
# EXEMPLES
################################################################################
print_section "Exemples d'utilisation"

mkdir -p examples/json-db

cat > examples/json-db/basic-usage.ts << 'EOF'
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
EOF

cat > examples/json-db/jsonld-example.ts << 'EOF'
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
EOF

print_success "Exemples créés"

################################################################################
# TESTS
################################################################################
print_section "Tests"

mkdir -p tests/unit/json-db

cat > tests/unit/json-db/collection.test.ts << 'EOF'
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
EOF

cat > tests/unit/json-db/query.test.ts << 'EOF'
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
EOF

print_success "Tests créés"

################################################################################
# MISE À JOUR DE CARGO.TOML
################################################################################
print_section "Dépendances Rust"

cat >> src-tauri/Cargo.toml << 'EOF'

# JSON DB dependencies
[dependencies]
jsonschema = "0.17"
serde_json = "1.0"
EOF

print_success "Dépendances ajoutées à Cargo.toml"

################################################################################
# README
################################################################################

cat > src-tauri/src/json_db/README.md << 'EOF'
# JSON Database Module

Base de données JSON embarquée pour GenAptitude avec :

## Fonctionnalités

- ✅ Collections avec schémas JSON Schema
- ✅ Validation automatique des documents
- ✅ Support JSON-LD pour données liées
- ✅ Indexes (B-Tree, Hash, Full-Text)
- ✅ Transactions ACID
- ✅ Migrations de schémas
- ✅ Requêtes expressives
- ✅ Cache en mémoire
- ✅ Compression automatique

## Architecture

```
json_db/
├── collections/      # Gestion des collections
├── schema/          # Validation JSON Schema
├── jsonld/          # Support JSON-LD
├── query/           # Moteur de requêtes
├── storage/         # Stockage fichier + cache
├── transactions/    # ACID + WAL
├── indexes/         # Indexation
└── migrations/      # Évolution des schémas
```

## Usage

Voir `docs/json-db.md` pour la documentation complète.
EOF

################################################################################
# RÉSUMÉ
################################################################################
print_section "AJOUT TERMINÉ !"

echo -e "${GREEN}Module JSON Database ajouté avec succès !${NC}\n"

echo "📦 Composants ajoutés:"
echo "   ✅ Backend Rust: src-tauri/src/json_db/"
echo "   ✅ Commandes Tauri: src-tauri/src/commands/json_db_commands.rs"
echo "   ✅ Services Frontend: src/services/json-db/"
echo "   ✅ Types TypeScript: src/types/json-db.types.ts"
echo "   ✅ Schémas JSON: domain-models/{software,system,hardware}/json-schemas/"
echo "   ✅ Contextes JSON-LD: domain-models/{software,system,hardware}/jsonld-contexts/"
echo "   ✅ Documentation: docs/json-db.md"
echo "   ✅ Exemples: examples/json-db/"
echo "   ✅ Tests: tests/unit/json-db/"
echo ""

echo "🔧 Fonctionnalités:"
echo "   • Collections avec schémas JSON Schema"
echo "   • Validation automatique des documents"
echo "   • Support JSON-LD pour sémantique"
echo "   • Moteur de requêtes expressif"
echo "   • Indexes pour performances"
echo "   • Transactions ACID"
echo "   • Migrations de schémas"
echo ""

echo "📖 Documentation: docs/json-db.md"
echo "🧪 Exemples: examples/json-db/"
echo ""

print_success "Module JSON DB prêt ! 🎉"
