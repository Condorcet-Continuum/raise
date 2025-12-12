import type { DragEvent } from 'react';

export function NodeLibrary() {
  const nodeTypes = [
    { id: 'trigger', label: 'Déclencheur', icon: '⚡', color: 'var(--color-warning)' },
    { id: 'action', label: 'Action Script', icon: '⚙️', color: 'var(--color-primary)' },
    { id: 'condition', label: 'Condition If/Else', icon: '🔀', color: 'var(--color-accent)' },
    { id: 'api', label: 'Appel API', icon: '🌐', color: 'var(--color-info)' },
    { id: 'end', label: 'Terminaison', icon: '🛑', color: 'var(--color-error)' },
  ];

  const handleDragStart = (e: DragEvent, type: string) => {
    e.dataTransfer.setData('workflowNodeType', type);
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
        zIndex: 10,
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
        }}
      >
        Boîte à Outils
      </header>

      <div
        style={{
          padding: 'var(--spacing-4)',
          display: 'flex',
          flexDirection: 'column',
          gap: 'var(--spacing-2)',
        }}
      >
        {nodeTypes.map((node) => (
          <div
            key={node.id}
            draggable
            onDragStart={(e) => handleDragStart(e, node.id)}
            style={{
              padding: 'var(--spacing-3)',
              backgroundColor: 'var(--bg-app)',
              border: '1px solid var(--border-color)',
              borderRadius: 'var(--radius-md)',
              cursor: 'grab',
              display: 'flex',
              alignItems: 'center',
              gap: 'var(--spacing-3)',
              color: 'var(--text-main)',
              fontSize: 'var(--font-size-sm)',
              borderLeft: `4px solid ${node.color}`,
              transition: 'transform 0.2s',
            }}
            onMouseEnter={(e) => (e.currentTarget.style.transform = 'translateX(4px)')}
            onMouseLeave={(e) => (e.currentTarget.style.transform = 'translateX(0)')}
          >
            <span>{node.icon}</span>
            <span>{node.label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
