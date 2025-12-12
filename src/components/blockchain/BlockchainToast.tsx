import { useEffect, useState } from 'react';

export const BlockchainToast = ({ trigger }: { trigger: boolean }) => {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    if (trigger) {
      setVisible(true);
      // La notification disparaît toute seule après 8 secondes
      const timer = setTimeout(() => setVisible(false), 8000);
      return () => clearTimeout(timer);
    }
  }, [trigger]);

  if (!visible) return null;

  return (
    <>
      <style>
        {`
          @keyframes slideUp {
            from { transform: translateY(100%); opacity: 0; }
            to { transform: translateY(0); opacity: 1; }
          }
          @keyframes pulse-success {
            0% { box-shadow: 0 0 0 0 var(--color-success); }
            70% { box-shadow: 0 0 0 6px transparent; }
            100% { box-shadow: 0 0 0 0 transparent; }
          }
        `}
      </style>
      <div
        style={{
          position: 'fixed',
          bottom: 'var(--spacing-8)',
          right: 'var(--spacing-8)',
          zIndex: 'var(--z-index-tooltip)',

          // Style adaptable (Light/Dark)
          backgroundColor: 'var(--bg-panel)',
          border: '1px solid var(--color-success)',
          color: 'var(--text-main)',

          padding: 'var(--spacing-4)',
          borderRadius: 'var(--radius-md)',
          boxShadow: 'var(--shadow-xl)',

          // Police technique pour le côté "Blockchain"
          fontFamily: 'var(--font-family-mono)',

          display: 'flex',
          alignItems: 'center',
          gap: 'var(--spacing-4)',
          minWidth: '400px',

          // Animation
          animation: 'slideUp 0.6s cubic-bezier(0.16, 1, 0.3, 1), pulse-success 2s infinite',
        }}
      >
        {/* Icône de chaîne / Blockchain */}
        <div
          style={{
            fontSize: '1.5rem',
            // Fond vert léger pour l'icône, texte vert
            backgroundColor: 'rgba(16, 185, 129, 0.1)',
            color: 'var(--color-success)',
            borderRadius: 'var(--radius-full)',
            width: '48px',
            height: '48px',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            flexShrink: 0,
          }}
        >
          🔗
        </div>

        <div style={{ flex: 1 }}>
          <div
            style={{
              textTransform: 'uppercase',
              fontWeight: 'var(--font-weight-bold)',
              letterSpacing: '1px',
              fontSize: 'var(--font-size-sm)',
              marginBottom: 'var(--spacing-1)',
              color: 'var(--color-success)',
            }}
          >
            Blockchain Consensus Reached
          </div>

          <div
            style={{
              fontSize: 'var(--font-size-xs)',
              color: 'var(--text-muted)',
              marginBottom: 'var(--spacing-1)',
            }}
          >
            Decision Hash:{' '}
            <span
              style={{
                fontWeight: 'bold',
                color: 'var(--text-main)',
              }}
            >
              0x8f4c...e2a4
            </span>
          </div>

          <div
            style={{
              fontSize: '0.7rem',
              backgroundColor: 'var(--color-success)',
              color: '#ffffff', // Toujours blanc sur fond vert
              padding: '2px 8px',
              borderRadius: 'var(--radius-sm)',
              display: 'inline-block',
              marginTop: 'var(--spacing-1)',
              fontWeight: 'var(--font-weight-medium)',
            }}
          >
            ✓ Anchored on Hyperledger Fabric
          </div>
        </div>
      </div>
    </>
  );
};
