interface SuggestionPanelProps {
  suggestions: string[];
  onSelect: (value: string) => void;
}

export function SuggestionPanel({ suggestions, onSelect }: SuggestionPanelProps) {
  if (!suggestions.length) return null;

  return (
    <div
      style={{
        display: 'flex',
        flexWrap: 'wrap',
        gap: 'var(--spacing-2)',
        marginBottom: 'var(--spacing-4)',
      }}
    >
      {suggestions.map((s) => (
        <button
          key={s}
          type="button"
          onClick={() => onSelect(s)}
          style={{
            borderRadius: 'var(--radius-full)',
            padding: '4px 12px',
            border: '1px solid var(--border-color)',
            backgroundColor: 'var(--color-gray-50)',
            color: 'var(--text-muted)',
            fontSize: 'var(--font-size-xs)',
            cursor: 'pointer',
            transition: 'all 0.2s',
          }}
          // Ajout d'un effet hover simple via style callback si on utilisait styled-components
          // Ici on s'appuie sur le CSS global ou on laisse simple.
        >
          {s}
        </button>
      ))}
    </div>
  );
}
