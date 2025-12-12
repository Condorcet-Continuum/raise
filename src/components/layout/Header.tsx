import { ThemeToggle } from '../shared/ThemeToggle';

interface HeaderProps {
  title: string;
}

export function Header({ title }: HeaderProps) {
  return (
    <header
      style={{
        height: 'var(--header-height)',
        backgroundColor: 'var(--bg-panel)',
        borderBottom: '1px solid var(--border-color)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        padding: '0 var(--spacing-6)',
        gap: 'var(--spacing-4)', // Espacement de sécurité
      }}
    >
      {/* Conteneur Titre : prend la place dispo mais tronque si trop long */}
      <div style={{ flex: 1, minWidth: 0, display: 'flex' }}>
        <h1
          style={{
            fontSize: 'var(--font-size-lg)',
            margin: 0,
            color: 'var(--text-main)',
            whiteSpace: 'nowrap',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
          }}
          title={title} // Tooltip natif pour lire le titre entier
        >
          {title}
        </h1>
      </div>

      {/* Conteneur Actions : ne rétrécit jamais */}
      <div style={{ flexShrink: 0, display: 'flex', alignItems: 'center' }}>
        <ThemeToggle />
      </div>
    </header>
  );
}
