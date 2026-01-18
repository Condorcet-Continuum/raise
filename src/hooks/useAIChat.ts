import { useCallback } from 'react';
// On remplace l'invoke direct par le service centralisé
import { aiService } from '@/services/ai-service';
import { useAiStore } from '@/store/ai-store';
import { ChatMessage } from '@/types/ai.types';

export function useAIChat() {
  // On récupère les actions du store global
  const {
    messages,
    isThinking,
    error,
    addMessage,
    setThinking,
    setError,
    clear: clearStore,
  } = useAiStore();

  const sendMessage = useCallback(
    async (text: string) => {
      // Nettoyage de l'input
      if (!text.trim()) return;

      // 1. On affiche tout de suite le message de l'utilisateur
      const userMsg: ChatMessage = {
        id: Date.now().toString(),
        role: 'user',
        content: text,
        createdAt: new Date().toISOString(),
      };
      addMessage(userMsg);

      setThinking(true);
      setError(undefined);

      try {
        console.log('🚀 Envoi vers Rust via aiService...');

        // 2. Appel Backend via le Service (plus propre que invoke direct)
        const response = await aiService.chat(text);

        // 3. On crée la réponse de l'assistant
        const aiMsg: ChatMessage = {
          id: (Date.now() + 1).toString(),
          role: 'assistant',
          content: response.content,
          // Les artefacts sont optionnels, on sécurise avec || []
          artifacts: response.artifacts || [],
          createdAt: new Date().toISOString(),
        };
        addMessage(aiMsg);
      } catch (err) {
        console.error('❌ Erreur AI:', err);
        const errorString = err instanceof Error ? err.message : String(err);
        setError(errorString);

        const errorMsg: ChatMessage = {
          id: Date.now().toString(),
          role: 'assistant',
          content: `⚠️ Erreur système : ${errorString}`,
          createdAt: new Date().toISOString(),
        };
        addMessage(errorMsg);
      } finally {
        setThinking(false);
      }
    },
    [addMessage, setThinking, setError],
  );

  // 4. Nouvelle fonction de nettoyage qui vide le Backend ET le Frontend
  const clear = useCallback(async () => {
    try {
      // On vide la mémoire conversationnelle du côté Rust
      await aiService.resetMemory();
      // Puis on vide l'affichage côté React
      clearStore();
    } catch (e) {
      console.error('Erreur lors du reset mémoire:', e);
      // On vide quand même le store local pour ne pas bloquer l'UI
      clearStore();
    }
  }, [clearStore]);

  // 5. --- NOUVELLE FONCTION : FEEDBACK UTILISATEUR ---
  // Permet à l'UI de valider une action et d'entraîner le World Model
  const confirmLearning = useCallback(
    async (actionIntent: 'Create' | 'Delete', entityName: string, entityKind: string) => {
      try {
        await aiService.confirmLearning(actionIntent, entityName, entityKind);
        // On pourrait ajouter ici un toast ou une notification de succès
      } catch (err) {
        console.error("Erreur lors de l'apprentissage:", err);
      }
    },
    [],
  );

  return {
    messages,
    isThinking,
    error,
    sendMessage,
    clear,
    confirmLearning, // <--- Exposée pour les composants
  };
}
