// FICHIER : src/services/tauri-commands.ts

import { invoke } from '@tauri-apps/api/core';

/**
 * Registre central des commandes Tauri (Rust).
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

  // --- MODULE WORKFLOW / GOUVERNANCE ---
  WORKFLOW_SUBMIT: 'submit_mandate',
  WORKFLOW_START: 'start_workflow',
  WORKFLOW_RESUME: 'resume_workflow',
  WORKFLOW_STATE: 'get_workflow_state',

  // Contrôle Jumeau Numérique
  SENSOR_SET: 'set_sensor_value',

  // --- MODULE SPATIAL (Nouveau) ---
  SPATIAL_TOPOLOGY: 'get_spatial_topology',
} as const;

// --- DÉFINITIONS DES TYPES ---

export type ExecutionStatus = 'Pending' | 'Running' | 'Completed' | 'Failed' | 'Paused' | 'Skipped';

export interface Mandate {
  id?: string;
  meta: {
    author: string;
    version: string;
    status: string;
  };
  governance: {
    strategy: string;
    condorcetWeights?: Record<string, number>; // camelCase obligatoire
  };
  hardLogic: {
    vetos: Array<{
      rule: string;
      active: boolean;
      action: string;
    }>;
  };
  observability: {
    heartbeatMs: number;
    metrics: string[];
  };
  signature?: string | null;
}

export interface WorkflowView {
  id: string;
  status: ExecutionStatus;
  current_nodes: string[];
  logs: string[];
}

// --- WRAPPERS ---

export const setSensorValue = async (value: number): Promise<string> => {
  return await invoke<string>(CMDS.SENSOR_SET, { value });
};

export const submitMandate = async (mandate: Mandate): Promise<string> => {
  return await invoke<string>(CMDS.WORKFLOW_SUBMIT, { mandate });
};

export const startWorkflow = async (workflowId: string): Promise<WorkflowView> => {
  return await invoke<WorkflowView>(CMDS.WORKFLOW_START, { workflowId });
};

export const getWorkflowState = async (instanceId: string): Promise<WorkflowView> => {
  return await invoke<WorkflowView>(CMDS.WORKFLOW_STATE, { instanceId });
};

export const resumeWorkflow = async (instanceId: string): Promise<WorkflowView> => {
  return await invoke<WorkflowView>(CMDS.WORKFLOW_RESUME, { instanceId });
};
