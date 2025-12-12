import { SyntaxHighlighter } from './SyntaxHighlighter';

interface LivePreviewProps {
  content: string;
  format?: 'json' | 'text';
}

export function LivePreview({ content, format = 'json' }: LivePreviewProps) {
  let displayContent = content;
  let isValid = true;

  // On essaie de formater le JSON pour qu'il soit joli
  if (format === 'json') {
    try {
      const parsed = JSON.parse(content);
      displayContent = JSON.stringify(parsed, null, 2);
    } catch {
      isValid = false;
    }
  }

  return (
    <div
      style={{
        height: '100%',
        backgroundColor: 'var(--bg-app)', // Fond légèrement différent de l'éditeur
        borderLeft: '1px solid var(--border-color)',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <header
        style={{
          padding: 'var(--spacing-2) var(--spacing-4)',
          borderBottom: '1px solid var(--border-color)',
          fontSize: 'var(--font-size-xs)',
          fontWeight: 'var(--font-weight-bold)',
          color: 'var(--text-muted)',
          textTransform: 'uppercase',
          display: 'flex',
          justifyContent: 'space-between',
        }}
      >
        <span>Aperçu en direct</span>
        {!isValid && format === 'json' && (
          <span style={{ color: 'var(--color-error)' }}>JSON Invalide</span>
        )}
      </header>

      <div style={{ padding: 'var(--spacing-4)', overflow: 'auto', flex: 1 }}>
        {isValid ? (
          <SyntaxHighlighter code={displayContent} language={format} />
        ) : (
          <div style={{ color: 'var(--text-muted)', fontSize: 'var(--font-size-sm)' }}>
            En attente de contenu valide...
          </div>
        )}
      </div>
    </div>
  );
}
