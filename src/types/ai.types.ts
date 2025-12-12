// FICHIER : src/types/ai.types.ts

// --- CHAT ---

export type AiRole = 'user' | 'assistant' | 'system';

export interface ChatMessage {
  id: string;
  role: AiRole;
  content: string;
  createdAt: string;
  meta?: Record<string, unknown>;
}

// --- SYSTÈME & STATUS ---

export interface AiStatus {
  llm_connected: boolean;
  llm_model: string;
  context_documents: number;
  active_agents: string[];
}

// --- NLP ---

export interface NlpResult {
  token_count: number;
  tokens: string[];
  entities?: Array<{ text: string; label: string }>;
}

// --- CONFIGURATION ---

export type AiBackendType = 'mock' | 'tauri-local' | 'remote-api';
