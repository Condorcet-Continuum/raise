import type { DragEvent } from 'react';

export function ShapeLibrary() {
  const shapes = [
    { id: 'block', label: 'Bloc SysML', icon: '📦' },
    { id: 'actor', label: 'Acteur', icon: '👤' },
    { id: 'interface', label: 'Interface', icon: 'qa' }, // 'qa' simulé pour l'exemple
    { id: 'db', label: 'Base de données', icon: '🗄️' },
  ];

  const handleDragStart = (e: DragEvent, type: string) => {
    e.dataTransfer.setData('shapeType', type);
    e.dataTransfer.effectAllowed = 'copy';
  };

  return (
    <div
      style={{
        width: '240px',
        backgroundColor: 'var(--bg-panel)',
        borderRight: '1px solid var(--border-color)',
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
      }}
    >
      <header
        style={{
          padding: 'var(--spacing-4)',
          borderBottom: '1px solid var(--border-color)',
          fontSize: 'var(--font-size-sm)',
          fontWeight: 'var(--font-weight-bold)',
          color: 'var(--text-main)',
          textTransform: 'uppercase',
          letterSpacing: '0.5px',
        }}
      >
        Bibliothèque
      </header>

      <div style={{ padding: 'var(--spacing-4)', display: 'grid', gap: 'var(--spacing-2)' }}>
        {shapes.map((shape) => (
          <div
            key={shape.id}
            draggable
            onDragStart={(e) => handleDragStart(e, shape.id)}
            style={{
              padding: 'var(--spacing-2) var(--spacing-4)',
              backgroundColor: 'var(--bg-app)', // Contraste léger
              border: '1px solid var(--border-color)',
              borderRadius: 'var(--radius-md)',
              cursor: 'grab',
              display: 'flex',
              alignItems: 'center',
              gap: 'var(--spacing-2)',
              color: 'var(--text-main)',
              fontSize: 'var(--font-size-sm)',
              transition: 'all 0.2s',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.borderColor = 'var(--color-primary)';
              e.currentTarget.style.color = 'var(--color-primary)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.borderColor = 'var(--border-color)';
              e.currentTarget.style.color = 'var(--text-main)';
            }}
          >
            <span style={{ fontSize: '1.2em' }}>{shape.icon}</span>
            <span>{shape.label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
