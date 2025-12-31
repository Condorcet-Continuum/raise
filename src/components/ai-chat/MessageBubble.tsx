import { ChatMessage, CreatedArtifact } from '@/types/ai.types';
import { ArtifactCard } from './ArtifactCard';

interface MessageBubbleProps {
  message: ChatMessage;
  // NOUVEAU : On accepte une fonction pour gérer le clic sur "Générer Code"
  onGenerateCode?: (language: string, artifact: CreatedArtifact) => void;
}

export function MessageBubble({ message, onGenerateCode }: MessageBubbleProps) {
  const isUser = message.role === 'user';
  const hasArtifacts = message.artifacts && message.artifacts.length > 0;

  return (
    <div
      className="ga-chat-bubble"
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: isUser ? 'flex-end' : 'flex-start',
        marginBottom: 'var(--spacing-2)',
        maxWidth: '85%',
        alignSelf: isUser ? 'flex-end' : 'flex-start',
      }}
    >
      {/* 1. Bulle de Texte */}
      <div
        style={{
          padding: 'var(--spacing-2) var(--spacing-4)',
          borderRadius: 'var(--radius-lg)',
          backgroundColor: isUser ? 'var(--color-primary)' : 'var(--color-gray-100)',
          color: isUser ? '#ffffff' : 'var(--text-main)',
          fontSize: 'var(--font-size-sm)',
          lineHeight: 'var(--line-height-relaxed)',
          whiteSpace: 'pre-wrap',
          boxShadow: 'var(--shadow-sm)',
          width: '100%',
        }}
      >
        {message.content}
      </div>

      {/* 2. Cartes d'Artefacts (Assistant uniquement) */}
      {!isUser && hasArtifacts && (
        <div style={{ marginTop: '8px', width: '100%', minWidth: '300px' }}>
          {message.artifacts!.map((art) => (
            <ArtifactCard
              key={art.id}
              artifact={art}
              onClick={(path) => console.log('Navigation vers :', path)}
              // C'est ici qu'on branche le tuyau !
              onGenerateCode={onGenerateCode}
            />
          ))}
        </div>
      )}

      {/* 3. Méta-données */}
      <div
        style={{
          fontSize: '0.7rem',
          color: 'var(--text-muted)',
          marginTop: 'var(--spacing-1)',
          padding: '0 4px',
          alignSelf: isUser ? 'flex-end' : 'flex-start',
        }}
      >
        {isUser ? 'Vous' : 'RAISE'} ·{' '}
        {new Date(message.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
      </div>
    </div>
  );
}
