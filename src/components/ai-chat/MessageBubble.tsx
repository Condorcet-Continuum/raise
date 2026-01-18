import { ChatMessage, CreatedArtifact } from '@/types/ai.types';
import { ArtifactCard } from './ArtifactCard';

interface MessageBubbleProps {
  message: ChatMessage;
  // Callback pour la génération de code
  onGenerateCode?: (language: string, artifact: CreatedArtifact) => void;
  // Callback pour le feedback au World Model
  onConfirmLearning?: (intent: 'Create' | 'Delete', name: string, kind: string) => void;
}

export function MessageBubble({ message, onGenerateCode, onConfirmLearning }: MessageBubbleProps) {
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
            <div key={art.id} style={{ marginBottom: '8px' }}>
              <ArtifactCard
                artifact={art}
                onClick={(path) => console.log('Navigation vers :', path)}
                onGenerateCode={onGenerateCode}
              />

              {/* --- ZONE DE FEEDBACK WORLD MODEL --- */}
              {onConfirmLearning && (
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '8px',
                    marginTop: '4px',
                    padding: '4px 8px',
                    backgroundColor: 'rgba(0, 255, 0, 0.05)',
                    border: '1px solid rgba(0, 255, 0, 0.1)',
                    borderRadius: '4px',
                    fontSize: '0.7rem',
                    color: 'var(--text-muted)',
                  }}
                >
                  <span>🧠 World Model :</span>
                  <button
                    // CORRECTION ICI : art.type -> art.element_type
                    onClick={() =>
                      onConfirmLearning('Create', art.name, art.element_type || 'Unknown')
                    }
                    style={{
                      background: 'none',
                      border: 'none',
                      cursor: 'pointer',
                      color: 'var(--color-success)',
                      fontWeight: 'bold',
                      padding: '2px 6px',
                      borderRadius: '4px',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '4px',
                    }}
                    title="Confirmer que cette action est valide pour entraîner le modèle"
                  >
                    ✅ Valider l'impact
                  </button>
                </div>
              )}
            </div>
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
