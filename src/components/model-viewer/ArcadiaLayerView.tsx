interface ArcadiaLayerViewProps {
  activeLayer: string;
  onLayerSelect: (layer: string) => void;
}

export function ArcadiaLayerView({ activeLayer, onLayerSelect }: ArcadiaLayerViewProps) {
  const layers = [
    { id: 'oa', label: 'OA', full: 'Operational Analysis', color: '#f59e0b' },
    { id: 'sa', label: 'SA', full: 'System Analysis', color: '#10b981' },
    { id: 'la', label: 'LA', full: 'Logical Arch.', color: '#3b82f6' },
    { id: 'pa', label: 'PA', full: 'Physical Arch.', color: '#8b5cf6' },
    { id: 'epbs', label: 'EPBS', full: 'Product Breakdown', color: '#db2777' },
  ];

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 'var(--spacing-2)',
        padding: 'var(--spacing-2)',
        backgroundColor: 'var(--bg-panel)',
        borderRight: '1px solid var(--border-color)',
        height: '100%',
        width: '60px', // Vue compacte
        alignItems: 'center',
      }}
    >
      <div
        style={{
          fontSize: '0.7rem',
          fontWeight: 'bold',
          color: 'var(--text-muted)',
          marginBottom: 'var(--spacing-2)',
          textAlign: 'center',
        }}
      >
        LAYERS
      </div>

      {layers.map((layer) => {
        const isActive = activeLayer === layer.id;
        return (
          <button
            key={layer.id}
            onClick={() => onLayerSelect(layer.id)}
            title={layer.full}
            style={{
              width: '40px',
              height: '40px',
              borderRadius: 'var(--radius-full)',
              border: `2px solid ${isActive ? layer.color : 'transparent'}`,
              backgroundColor: isActive ? layer.color : 'var(--bg-app)',
              color: isActive ? '#fff' : 'var(--text-muted)',
              fontWeight: 'bold',
              cursor: 'pointer',
              fontSize: '0.8rem',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              transition: 'all 0.2s',
              boxShadow: isActive ? `0 0 10px ${layer.color}66` : 'none',
            }}
          >
            {layer.label}
          </button>
        );
      })}
    </div>
  );
}
