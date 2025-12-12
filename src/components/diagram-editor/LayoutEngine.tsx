export function LayoutEngine() {
  const runLayout = (direction: 'TB' | 'LR') => {
    console.log(`Réorganisation automatique : ${direction}`);
    // Ici on appellerait un algo type Dagre ou Elkjs
  };

  return (
    <div
      style={{
        position: 'absolute',
        bottom: 'var(--spacing-4)',
        right: 'var(--spacing-4)',
        backgroundColor: 'var(--bg-panel)',
        padding: 'var(--spacing-2)',
        borderRadius: 'var(--radius-md)',
        border: '1px solid var(--border-color)',
        boxShadow: 'var(--shadow-md)',
        zIndex: 'var(--z-index-sticky)',
        display: 'flex',
        flexDirection: 'column',
        gap: 'var(--spacing-2)',
      }}
    >
      <span
        style={{
          fontSize: 'var(--font-size-xs)',
          color: 'var(--text-muted)',
          fontWeight: 'bold',
          textAlign: 'center',
        }}
      >
        AUTO-LAYOUT
      </span>

      <div style={{ display: 'flex', gap: 'var(--spacing-2)' }}>
        <button
          onClick={() => runLayout('TB')}
          style={{
            padding: '4px 8px',
            backgroundColor: 'var(--bg-app)',
            border: '1px solid var(--border-color)',
            borderRadius: 'var(--radius-sm)',
            cursor: 'pointer',
            color: 'var(--text-main)',
            fontSize: 'var(--font-size-xs)',
          }}
        >
          ↕ Vertical
        </button>
        <button
          onClick={() => runLayout('LR')}
          style={{
            padding: '4px 8px',
            backgroundColor: 'var(--bg-app)',
            border: '1px solid var(--border-color)',
            borderRadius: 'var(--radius-sm)',
            cursor: 'pointer',
            color: 'var(--text-main)',
            fontSize: 'var(--font-size-xs)',
          }}
        >
          ↔ Horizontal
        </button>
      </div>
    </div>
  );
}
