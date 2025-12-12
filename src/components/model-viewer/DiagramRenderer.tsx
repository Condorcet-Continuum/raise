interface DiagramRendererProps {
  diagramId?: string;
}

export function DiagramRenderer({ diagramId }: DiagramRendererProps) {
  return (
    <div
      style={{
        width: '100%',
        height: '100%',
        backgroundColor: 'var(--bg-app)', // Fond de la zone de dessin
        backgroundImage: 'radial-gradient(var(--border-color) 1px, transparent 1px)',
        backgroundSize: '20px 20px',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        position: 'relative',
        overflow: 'hidden',
      }}
    >
      {diagramId ? (
        <div style={{ textAlign: 'center' }}>
          {/* Simulation d'un diagramme */}
          <div
            style={{
              width: 400,
              height: 300,
              border: '2px solid var(--color-primary)',
              backgroundColor: 'var(--bg-panel)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              borderRadius: 'var(--radius-md)',
              boxShadow: 'var(--shadow-lg)',
            }}
          >
            <span style={{ color: 'var(--text-main)' }}>Diagramme: {diagramId}</span>
          </div>
        </div>
      ) : (
        <div style={{ color: 'var(--text-muted)' }}>
          Sélectionnez un élément pour voir son diagramme
        </div>
      )}

      <div
        style={{
          position: 'absolute',
          bottom: 'var(--spacing-4)',
          right: 'var(--spacing-4)',
          padding: 'var(--spacing-2)',
          backgroundColor: 'var(--bg-panel)',
          border: '1px solid var(--border-color)',
          borderRadius: 'var(--radius-sm)',
          fontSize: 'var(--font-size-xs)',
          color: 'var(--text-muted)',
        }}
      >
        Zoom: 100%
      </div>
    </div>
  );
}
