/**
 * Registre central des commandes Tauri (Rust).
 * Utiliser ces constantes dans invoke() évite les typos.
 */
export const CMDS = {
  // Module AI
  AI_CHAT: 'ai_chat',
  AI_STATUS: 'ai_get_system_status',
  AI_TEST_NLP: 'ai_test_nlp',

  // Module JsonDB
  DB_CREATE_DB: 'jsondb_create_db',
  DB_CREATE_COLLECTION: 'jsondb_create_collection',
  DB_INSERT: 'jsondb_insert_document',
  DB_QUERY: 'jsondb_execute_query',
  DB_SQL: 'jsondb_execute_sql',

  // Module Model
  MODEL_LOAD: 'load_project_model',
  MODEL_SAVE: 'save_project_model',

  // Module Codegen
  CODEGEN: 'generate_source_code',

  // Module Genetics
  GENETICS_RUN: 'run_genetic_optimization',

  // Module Cognitive
  COGNITIVE_ANALYZE: 'run_consistency_analysis',
} as const;
