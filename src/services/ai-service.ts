import { invoke } from '@tauri-apps/api/core';
import type { CreatedArtifact } from '@/types/ai.types';

// Structure retournée par le Backend Rust (AgentResult)
export interface AgentResult {
  type: 'text' | 'action' | 'file';
  content: string;
  artifacts?: CreatedArtifact[];
  metadata?: Record<string, unknown>;
}

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
   * Envoie un message à l'Orchestrateur IA.
   */
  async chat(userInput: string): Promise<AgentResult> {
    try {
      // Note : Tauri convertit automatiquement userInput -> user_input
      return await invoke<AgentResult>('ai_chat', {
        userInput,
      });
    } catch (error) {
      console.error('[AiService] Chat error:', error);
      throw error;
    }
  }

  /**
   * --- NOUVEAU ---
   * Envoie un signal de renforcement (feedback) au World Model.
   * Permet au cerveau d'apprendre que cette action était la bonne.
   */
  async confirmLearning(
    actionIntent: 'Create' | 'Delete',
    entityName: string,
    entityKind: string,
  ): Promise<string> {
    try {
      console.log(
        `[AiService] 🧠 Envoi du feedback d'apprentissage : ${actionIntent} -> ${entityName}`,
      );

      // Appel vers src-tauri/src/commands/ai_commands.rs -> ai_confirm_learning
      // IMPORTANT : On map explicitement les clés pour matcher les args Rust (snake_case)
      const result = await invoke<string>('ai_confirm_learning', {
        action_intent: actionIntent,
        entity_name: entityName,
        entity_kind: entityKind,
      });

      console.log('[AiService] ✅ Cerveau mis à jour:', result);
      return result;
    } catch (error) {
      console.error('[AiService] Learning error:', error);
      // On retourne une chaîne d'erreur pour que l'UI puisse l'afficher si besoin
      return `Erreur d'apprentissage: ${error}`;
    }
  }

  /**
   * Réinitialise la mémoire conversationnelle côté Backend.
   */
  async resetMemory(): Promise<void> {
    try {
      await invoke('ai_reset');
      console.log('[AiService] Mémoire réinitialisée.');
    } catch (error) {
      console.error('[AiService] Reset error:', error);
      throw error;
    }
  }

  /**
   * Récupère le statut (ou un mock si la commande n'est pas encore implémentée).
   */
  async getSystemStatus(): Promise<AiStatus> {
    try {
      return await invoke<AiStatus>('ai_get_system_status');
    } catch (error) {
      console.warn('[AiService] Status command not found (using mock). details:', error);

      return {
        llm_connected: true,
        llm_model: 'Llama-3-Local',
        context_documents: 12,
        active_agents: ['Orchestrator', 'WorldModel'],
      };
    }
  }

  async testNlp(text: string): Promise<NlpResult> {
    try {
      return await invoke<NlpResult>('ai_test_nlp', { text });
    } catch {
      return { token_count: 0, tokens: [] };
    }
  }
}

export const aiService = new AiService();
