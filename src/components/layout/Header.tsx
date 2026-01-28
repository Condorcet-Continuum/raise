import { ThemeToggle } from '../shared/ThemeToggle';
import { useUiStore } from '../../store/ui-store';
import React from 'react';

interface HeaderProps {
  title: string;
}

export function Header({ title }: HeaderProps) {
  // Connexion au store pour le bouton 3D
  const { viewMode, setViewMode } = useUiStore();

  // Style "Hardcoded" pour garantir la stabilité du Layout
  const headerStyle: React.CSSProperties = {
    height: '64px', // Hauteur fixe (plus sûr que var(--header-height))
    width: '100%', // Prend toute la largeur
    flexShrink: 0, // INTERDIT de s'écraser si la fenêtre est petite
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    padding: '0 24px',
    backgroundColor: 'var(--bg-panel)',
    borderBottom: '1px solid var(--border-color)',
    boxSizing: 'border-box', // Important pour le padding
  };

  // Style du bouton 3D
  const buttonStyle: React.CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    gap: '8px',
    padding: '6px 12px',
    borderRadius: '6px',
    fontSize: '0.85rem',
    fontWeight: 600,
    border: '1px solid',
    cursor: 'pointer',
    transition: 'all 0.2s ease',
    backgroundColor: viewMode === '3d' ? 'rgba(34, 197, 94, 0.1)' : 'transparent',
    borderColor: viewMode === '3d' ? '#22c55e' : 'var(--border-color, #ccc)',
    color: viewMode === '3d' ? '#22c55e' : 'var(--text-main, inherit)',
  };

  return (
    <header style={headerStyle}>
      {/* Titre */}
      <div style={{ flex: 1, minWidth: 0, display: 'flex' }}>
        <h1
          style={{
            fontSize: '1.125rem',
            fontWeight: 600,
            margin: 0,
            color: 'var(--text-main)',
            whiteSpace: 'nowrap',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
          }}
          title={title}
        >
          {title}
        </h1>
      </div>

      {/* Actions */}
      <div style={{ flexShrink: 0, display: 'flex', alignItems: 'center', gap: '12px' }}>
        {/* BOUTON 3D (Cible du test Playwright) */}
        <button
          data-testid="view-mode-3d"
          onClick={() => setViewMode(viewMode === '2d' ? '3d' : '2d')}
          style={buttonStyle}
          type="button"
        >
          <span
            style={{
              width: '8px',
              height: '8px',
              borderRadius: '50%',
              backgroundColor: viewMode === '3d' ? '#22c55e' : '#94a3b8',
            }}
          />
          {viewMode === '3d' ? '3D ACTIVE' : '3D VIEW'}
        </button>

        <ThemeToggle />
      </div>
    </header>
  );
}
