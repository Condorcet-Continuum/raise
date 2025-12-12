interface CodeCompletionProps {
  suggestions: string[];
  visible: boolean;
  position: { top: number; left: number };
  onSelect: (suggestion: string) => void;
  onClose: () => void;
}

export function CodeCompletion({ suggestions, visible, position, onSelect }: CodeCompletionProps) {
  if (!visible || suggestions.length === 0) return null;

  return (
    <div
      style={{
        position: 'absolute',
        top: position.top + 20, // Juste en dessous du curseur
        left: position.left,
        zIndex: 'var(--z-index-popover)',

        backgroundColor: 'var(--bg-panel)',
        border: '1px solid var(--border-color)',
        borderRadius: 'var(--radius-md)',
        boxShadow: 'var(--shadow-lg)',

        minWidth: 150,
        maxHeight: 200,
        overflowY: 'auto',
        padding: 'var(--spacing-1)',
      }}
    >
      <div
        style={{
          fontSize: '0.7rem',
          padding: '4px 8px',
          color: 'var(--text-muted)',
          borderBottom: '1px solid var(--border-color)',
          marginBottom: 4,
        }}
      >
        Suggestions
      </div>

      {suggestions.map((item, index) => (
        <button
          key={index}
          onClick={() => onSelect(item)}
          style={{
            display: 'block',
            width: '100%',
            textAlign: 'left',
            background: 'transparent',
            border: 'none',
            padding: '6px 8px',
            cursor: 'pointer',
            color: 'var(--text-main)',
            fontSize: 'var(--font-size-sm)',
            fontFamily: 'var(--font-family-mono)',
            borderRadius: 'var(--radius-sm)',
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.backgroundColor = 'var(--color-primary-light)';
            e.currentTarget.style.color = '#fff';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.backgroundColor = 'transparent';
            e.currentTarget.style.color = 'var(--text-main)';
          }}
        >
          {item}
        </button>
      ))}
    </div>
  );
}
