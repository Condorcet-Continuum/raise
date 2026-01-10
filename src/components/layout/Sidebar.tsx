import { useUiStore } from '@/store/ui-store';

interface SidebarProps {
  currentPage: string;
  onNavigate: (page: string) => void;
}

export function Sidebar({ currentPage, onNavigate }: SidebarProps) {
  const { sidebarOpen, toggleSidebar } = useUiStore();

  const mainMenuItems = [
    { id: 'dashboard', label: 'Tableau de bord', icon: '📊' },

    { id: 'governance', label: 'Gouvernance & Mandats', icon: '⚖️' },

    { id: 'model', label: 'Modélisation Arcadia', icon: '💠' },
    { id: 'diagram', label: 'Éditeur de Diagrammes', icon: '✏️' },
    { id: 'workflow', label: 'Workflow Designer', icon: '🔀' },

    { id: 'rules-engine', label: 'Moteur de Règles', icon: '🧮' },

    { id: 'genetics', label: 'Optimisation Génétique', icon: '🧬' },
    { id: 'cognitive', label: 'Moteur Cognitif', icon: '🧠' },
    { id: 'codegen', label: 'Génération de Code', icon: '⚙️' },
    { id: 'assurance', label: 'Qualité & XAI', icon: '🛡️' },

    { id: 'ai', label: 'MBAIE (AI Engineering)', icon: '🤖' },
    { id: 'blockchain', label: 'Blockchain', icon: '🔗' },
    { id: 'cognitive-tester', label: 'Testeur WASM', icon: '🧪' },
    { id: 'admin-db', label: 'Base de Données', icon: '🗄️' },
  ];

  const bottomMenuItems = [{ id: 'settings', label: 'Paramètres', icon: '⚙️' }];

  // Helper de rendu (inchangé)
  const renderMenuItem = (item: { id: string; label: string; icon: string }) => {
    const isActive = currentPage === item.id;
    return (
      <li key={item.id}>
        <button
          onClick={() => onNavigate(item.id)}
          title={!sidebarOpen ? item.label : ''}
          style={{
            width: '100%',
            display: 'flex',
            alignItems: 'center',
            justifyContent: sidebarOpen ? 'flex-start' : 'center',
            gap: sidebarOpen ? 'var(--spacing-3)' : 0,
            padding: '10px 12px',
            border: 'none',
            borderRadius: 'var(--radius-md)',
            backgroundColor: isActive ? 'var(--color-primary)' : 'transparent',
            color: isActive ? '#ffffff' : 'var(--text-muted)',
            cursor: 'pointer',
            fontSize: 'var(--font-size-sm)',
            fontWeight: isActive ? 'var(--font-weight-semibold)' : 'var(--font-weight-medium)',
            transition: 'all 0.2s',
            textAlign: 'left',
            whiteSpace: 'nowrap',
            overflow: 'hidden',
          }}
          onMouseEnter={(e) => {
            if (!isActive) {
              e.currentTarget.style.backgroundColor = 'var(--bg-app)';
              e.currentTarget.style.color = 'var(--text-main)';
            }
          }}
          onMouseLeave={(e) => {
            if (!isActive) {
              e.currentTarget.style.backgroundColor = 'transparent';
              e.currentTarget.style.color = 'var(--text-muted)';
            }
          }}
        >
          <span style={{ fontSize: '1.2rem', minWidth: '24px', textAlign: 'center' }}>
            {item.icon}
          </span>
          {sidebarOpen && (
            <span style={{ opacity: 1, transition: 'opacity 0.2s' }}>{item.label}</span>
          )}
        </button>
      </li>
    );
  };

  return (
    <aside
      style={{
        width: sidebarOpen ? 'var(--sidebar-width)' : '70px',
        backgroundColor: 'var(--bg-panel)',
        borderRight: '1px solid var(--border-color)',
        display: 'flex',
        flexDirection: 'column',
        height: '100vh',
        transition: 'width 0.3s cubic-bezier(0.2, 0, 0, 1)',
        zIndex: 'var(--z-index-fixed)',
        overflow: 'hidden',
      }}
    >
      {/* HEADER */}
      <div
        style={{
          height: 'var(--header-height)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: sidebarOpen ? 'flex-start' : 'center',
          padding: sidebarOpen ? '0 var(--spacing-6)' : '0',
          borderBottom: '1px solid var(--border-color)',
          fontSize: 'var(--font-size-lg)',
          fontWeight: 'var(--font-weight-bold)',
          color: 'var(--color-primary)',
          letterSpacing: '-0.5px',
          whiteSpace: 'nowrap',
        }}
      >
        {sidebarOpen ? (
          <>
            <span style={{ marginRight: '8px' }}>Gen</span>
            <span style={{ color: 'var(--text-main)' }}>Aptitude</span>
          </>
        ) : (
          <span>GA</span>
        )}
      </div>

      <nav style={{ flex: 1, padding: 'var(--spacing-2)', overflowY: 'auto', overflowX: 'hidden' }}>
        <ul
          style={{
            listStyle: 'none',
            padding: 0,
            margin: 0,
            display: 'flex',
            flexDirection: 'column',
            gap: '4px',
          }}
        >
          {mainMenuItems.map(renderMenuItem)}
        </ul>
      </nav>

      <div
        style={{
          padding: 'var(--spacing-2)',
          borderTop: '1px solid var(--border-color)',
          backgroundColor: 'var(--bg-panel)',
        }}
      >
        <ul style={{ listStyle: 'none', padding: 0, margin: '0 0 var(--spacing-2) 0' }}>
          {bottomMenuItems.map(renderMenuItem)}
        </ul>

        <button
          onClick={toggleSidebar}
          style={{
            width: '100%',
            padding: '8px',
            background: 'var(--bg-app)',
            border: '1px solid var(--border-color)',
            borderRadius: 'var(--radius-md)',
            color: 'var(--text-muted)',
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            transition: 'background 0.2s',
          }}
          title={sidebarOpen ? 'Réduire le menu' : 'Agrandir le menu'}
        >
          {sidebarOpen ? '◀ Réduire' : '▶'}
        </button>
      </div>
    </aside>
  );
}
