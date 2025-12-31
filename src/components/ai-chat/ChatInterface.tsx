import { useState } from 'react';
import { useAIChat } from '@/hooks/useAIChat';
import { MessageBubble } from './MessageBubble';
import { InputBar } from './InputBar';
import { SuggestionPanel } from './SuggestionPanel';
import { ContextDisplay } from './ContextDisplay';
import { IntentClassifier } from './IntentClassifier';
import { CreatedArtifact } from '@/types/ai.types';

export function ChatInterface() {
  // Le hook utilise maintenant le nouveau aiService compatible AgentResult
  const { messages, isThinking, error, sendMessage, clear } = useAIChat();
  const [input, setInput] = useState('');

  const lastMessage = messages[messages.length - 1];

  function handleSend(value: string) {
    setInput('');
    void sendMessage(value);
  }

  // --- Fonction intelligente de génération de code (Inchangée) ---
  const handleGenerateCode = (language: string, artifact: CreatedArtifact) => {
    const prompt = `Agis en tant qu'expert Software. 
    Génère le code ${language.toUpperCase()} complet pour l'élément "${artifact.name}" (Type: ${
      artifact.element_type
    }, Layer: ${artifact.layer}).`;
    void sendMessage(prompt);
  };

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        maxHeight: '100vh',
        backgroundColor: 'var(--bg-panel)',
        color: 'var(--text-main)',
        padding: 'var(--spacing-4)',
        borderRadius: 'var(--radius-lg)',
        border: '1px solid var(--border-color)',
        transition: 'var(--transition-base)',
      }}
    >
      {/* HEADER */}
      <header
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          marginBottom: 'var(--spacing-4)',
        }}
      >
        <div>
          <h2 style={{ fontSize: 'var(--font-size-lg)', margin: 0, color: 'var(--text-main)' }}>
            Assistant RAISE
          </h2>
          <ContextDisplay messagesCount={messages.length} />
        </div>

        {/* BOUTON RESET : Vide l'UI ET la mémoire Backend */}
        <button
          type="button"
          onClick={() => {
            if (confirm("Réinitialiser la mémoire de l'IA ?")) {
              clear();
            }
          }}
          style={{
            fontSize: 'var(--font-size-xs)',
            borderRadius: 'var(--radius-full)',
            border: '1px solid var(--border-color)',
            backgroundColor: 'var(--color-gray-50)',
            color: 'var(--text-muted)',
            padding: '4px 10px',
            cursor: 'pointer',
          }}
          title="Oublier la conversation et vider le contexte"
        >
          Effacer Mémoire
        </button>
      </header>

      <IntentClassifier lastMessage={lastMessage} />

      <SuggestionPanel
        suggestions={[
          "Quelle est l'autonomie du drone ? (RAG)",
          'Crée un LogicalComponent "FlightController"',
          'Analyse la cohérence du modèle',
        ]}
        onSelect={(val) => setInput(val)}
      />

      {/* ZONE DE MESSAGES */}
      <div style={{ flex: 1, overflowY: 'auto', padding: 'var(--spacing-2) 0' }}>
        {messages.length === 0 && (
          <div style={{ textAlign: 'center', color: 'var(--text-muted)', marginTop: '20%' }}>
            🤖{' '}
            <i>
              Je suis connecté au cerveau Rust.
              <br />
              Posez une question contextuelle ou demandez une création.
            </i>
          </div>
        )}

        {messages.map((m) => (
          <MessageBubble key={m.id} message={m} onGenerateCode={handleGenerateCode} />
        ))}

        {isThinking && (
          <div
            style={{
              fontSize: 'var(--font-size-xs)',
              color: 'var(--text-muted)',
              marginTop: 'var(--spacing-2)',
              fontStyle: 'italic',
              display: 'flex',
              alignItems: 'center',
              gap: '8px',
            }}
          >
            <span>L’assistant réfléchit…</span>
            {/* Petit loader CSS optionnel */}
            <span style={{ animation: 'pulse 1s infinite' }}>🧠</span>
          </div>
        )}

        {error && (
          <div
            style={{
              fontSize: 'var(--font-size-xs)',
              color: 'var(--color-error)',
              marginTop: 'var(--spacing-2)',
              padding: '8px',
              backgroundColor: '#fee2e2',
              borderRadius: '4px',
            }}
          >
            ⚠️ {error}
          </div>
        )}
      </div>

      <InputBar value={input} onChange={setInput} onSend={handleSend} disabled={isThinking} />
    </div>
  );
}
