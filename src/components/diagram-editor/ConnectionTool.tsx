import { useState } from 'react';

type ToolType = 'select' | 'connect' | 'text' | 'delete';

export function ConnectionTool() {
  const [activeTool, setActiveTool] = useState<ToolType>('select');

  const tools: { id: ToolType; icon: string; label: string }[] = [
    { id: 'select', icon: '↖', label: 'Sélection' },
    { id: 'connect', icon: '🔗', label: 'Lien' },
    { id: 'text', icon: 'T', label: 'Texte' },
    { id: 'delete', icon: '🗑', label: 'Supprimer' },
  ];

  return (
    <div
      style={{
        position: 'absolute',
        top: 'var(--spacing-4)',
        left: '50%',
        transform: 'translateX(-50%)',
        backgroundColor: 'var(--bg-panel)',
        padding: 'var(--spacing-1)',
        borderRadius: 'var(--radius-full)',
        border: '1px solid var(--border-color)',
        boxShadow: 'var(--shadow-lg)',
        display: 'flex',
        gap: 'var(--spacing-1)',
        zIndex: 'var(--z-index-sticky)',
      }}
    >
      {tools.map((tool) => {
        const isActive = activeTool === tool.id;
        return (
          <button
            key={tool.id}
            onClick={() => setActiveTool(tool.id)}
            title={tool.label}
            style={{
              width: '36px',
              height: '36px',
              borderRadius: '50%',
              border: 'none',
              cursor: 'pointer',
              fontSize: '1.2rem',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              backgroundColor: isActive ? 'var(--color-primary)' : 'transparent',
              color: isActive ? '#ffffff' : 'var(--text-main)',
              transition: 'var(--transition-fast)',
            }}
          >
            {tool.icon}
          </button>
        );
      })}
    </div>
  );
}
