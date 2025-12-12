import { useState } from 'react';
import { useAIChat } from '@/hooks/useAIChat';
import { MessageBubble } from './MessageBubble';
import { InputBar } from './InputBar';
import { SuggestionPanel } from './SuggestionPanel';
import { ContextDisplay } from './ContextDisplay';
import { IntentClassifier } from './IntentClassifier';

export function ChatInterface() {
  const { messages, isThinking, error, sendMessage, clear } = useAIChat();
  const [input, setInput] = useState('');

  const lastMessage = messages[messages.length - 1];

  function handleSend(value: string) {
    setInput('');
    void sendMessage(value);
  }

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        maxHeight: '100vh',
        // Utilisation des variables de thème
        backgroundColor: 'var(--bg-panel)',
        color: 'var(--text-main)',
        padding: 'var(--spacing-4)',
        borderRadius: 'var(--radius-lg)',
        border: '1px solid var(--border-color)',
        transition: 'var(--transition-base)',
      }}
    >
      <header
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          marginBottom: 'var(--spacing-4)',
        }}
      >
        <div>
          <h2
            style={{
              fontSize: 'var(--font-size-lg)',
              margin: 0,
              color: 'var(--text-main)',
            }}
          >
            Assistant GenAptitude
          </h2>
          <ContextDisplay messagesCount={messages.length} />
        </div>
        <button
          type="button"
          onClick={clear}
          style={{
            fontSize: 'var(--font-size-xs)',
            borderRadius: 'var(--radius-full)',
            border: '1px solid var(--border-color)',
            backgroundColor: 'var(--color-gray-50)', // Fond léger
            color: 'var(--text-muted)',
            padding: '4px 10px',
            cursor: 'pointer',
            transition: 'var(--transition-fast)',
          }}
        >
          Effacer
        </button>
      </header>

      <IntentClassifier lastMessage={lastMessage} />

      <SuggestionPanel
        suggestions={[
          'Explique-moi la structure JSON-DB actuelle',
          'Montre-moi un exemple de requête sur la collection "articles"',
          'Comment brancher Capella / Arcadia sur GenAptitude ?',
        ]}
        onSelect={setInput}
      />

      <div
        style={{
          flex: 1,
          overflowY: 'auto',
          padding: 'var(--spacing-2) 0',
        }}
      >
        {messages.map((m) => (
          <MessageBubble key={m.id} message={m} />
        ))}

        {isThinking && (
          <div
            style={{
              fontSize: 'var(--font-size-xs)',
              color: 'var(--text-muted)',
              marginTop: 'var(--spacing-2)',
              fontStyle: 'italic',
            }}
          >
            L’assistant réfléchit…
          </div>
        )}

        {error && (
          <div
            style={{
              fontSize: 'var(--font-size-xs)',
              color: 'var(--color-error)',
              marginTop: 'var(--spacing-2)',
            }}
          >
            Erreur : {error}
          </div>
        )}
      </div>

      <InputBar value={input} onChange={setInput} onSend={handleSend} disabled={isThinking} />
    </div>
  );
}
