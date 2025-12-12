import type { ChatMessage } from '@/store/ai-store';

interface MessageBubbleProps {
  message: ChatMessage;
}

export function MessageBubble({ message }: MessageBubbleProps) {
  const isUser = message.role === 'user';

  return (
    <div
      className="ga-chat-bubble"
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: isUser ? 'flex-end' : 'flex-start',
        marginBottom: 'var(--spacing-2)',
      }}
    >
      <div
        style={{
          maxWidth: '85%',
          padding: 'var(--spacing-2) var(--spacing-4)',
          borderRadius: 'var(--radius-lg)',
          // Logique de couleur :
          // User: Primaire (Bleu)
          // AI: Gris neutre qui s'adapte (Clair en Light, Foncé en Dark)
          backgroundColor: isUser ? 'var(--color-primary)' : 'var(--color-gray-100)',

          color: isUser
            ? '#ffffff' // Toujours blanc sur du bleu
            : 'var(--text-main)', // S'adapte au fond gris

          fontSize: 'var(--font-size-sm)',
          lineHeight: 'var(--line-height-relaxed)',
          whiteSpace: 'pre-wrap',
          boxShadow: 'var(--shadow-sm)',
        }}
      >
        {message.content}
      </div>
      <div
        style={{
          fontSize: '0.7rem',
          color: 'var(--text-muted)', // Gris discret
          marginTop: 'var(--spacing-1)',
          padding: '0 4px',
        }}
      >
        {isUser ? 'Vous' : 'Assistant'} · {new Date(message.createdAt).toLocaleTimeString()}
      </div>
    </div>
  );
}
