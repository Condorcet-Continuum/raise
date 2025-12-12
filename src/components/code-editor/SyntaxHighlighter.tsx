import type { ReactNode } from 'react';

interface SyntaxHighlighterProps {
  code: string;
  language?: 'json' | 'javascript' | 'text';
}

export function SyntaxHighlighter({ code, language = 'text' }: SyntaxHighlighterProps) {
  // Fonction simple pour colorer du JSON (Clés en primaire, chaînes en vert)
  const highlightJSON = (json: string): ReactNode[] => {
    const parts = json.split(/(".*?"|:|,|{|}|\[|\])/g);

    return parts.map((part, index) => {
      if (!part) return null;

      let color = 'var(--text-main)';
      let weight = 'var(--font-weight-normal)';

      // Les chaînes de caractères (entre guillemets)
      if (part.startsWith('"')) {
        // Si suivi de ':', c'est une clé
        if (parts[index + 1]?.trim() === ':') {
          color = 'var(--color-primary)'; // Clé JSON
          weight = 'var(--font-weight-semibold)';
        } else {
          color = 'var(--color-success)'; // Valeur String
        }
      }
      // Les booléens et null
      else if (['true', 'false', 'null'].includes(part.trim())) {
        color = 'var(--color-warning)';
      }
      // La ponctuation
      else if (['{', '}', '[', ']'].includes(part.trim())) {
        color = 'var(--color-accent)';
      }

      return (
        <span key={index} style={{ color, fontWeight: weight }}>
          {part}
        </span>
      );
    });
  };

  return (
    <pre
      style={{
        margin: 0,
        fontFamily: 'var(--font-family-mono)',
        fontSize: 'var(--font-size-sm)',
        lineHeight: '1.6',
        whiteSpace: 'pre-wrap',
        wordBreak: 'break-all',
        color: 'var(--text-main)',
      }}
    >
      <code>{language === 'json' ? highlightJSON(code) : code}</code>
    </pre>
  );
}
