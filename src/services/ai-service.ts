import { invoke } from '@tauri-apps/api/core';

export interface AiStatus {
  llm_connected: boolean;
  llm_model: string;
  context_documents: number;
  active_agents: string[];
}

export interface NlpResult {
  token_count: number;
  tokens: string[];
}

class AiService {
  /**
   * Envoie un message au Chatbot (LLM) via le backend Rust.
   * @param userInput Le message de l'utilisateur
   * @param systemPrompt (Optionnel) Instructions système pour guider l'IA
   */
  async chat(userInput: string, systemPrompt?: string): Promise<string> {
    try {
      return await invoke<string>('ai_chat', {
        userInput,
        systemPrompt,
      });
    } catch (error) {
      console.error('[AiService] Chat error:', error);
      throw error;
    }
  }

  /**
   * Récupère l'état global du système IA (connexion, modèle chargé...).
   */
  async getSystemStatus(): Promise<AiStatus> {
    try {
      return await invoke<AiStatus>('ai_get_system_status');
    } catch (error) {
      console.error('[AiService] Status error:', error);
      // Retour d'un état par défaut en cas d'erreur (mode dégradé)
      return {
        llm_connected: false,
        llm_model: 'Unknown',
        context_documents: 0,
        active_agents: [],
      };
    }
  }

  /**
   * Teste le moteur de tokenization NLP (utile pour le debugging).
   */
  async testNlp(text: string): Promise<NlpResult> {
    return await invoke<NlpResult>('ai_test_nlp', { text });
  }
}

export const aiService = new AiService();
