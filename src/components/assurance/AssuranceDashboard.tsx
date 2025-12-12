import { useState } from 'react';

export default function AssuranceDashboard() {
  const [activeTab, setActiveTab] = useState<'qa' | 'xai'>('qa');

  return (
    <div style={{ padding: 'var(--spacing-6)', color: 'var(--text-main)' }}>
      <header
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: 'var(--spacing-6)',
        }}
      >
        <div>
          <h2 style={{ margin: 0, color: 'var(--color-success)' }}>Product Assurance</h2>
          <p style={{ color: 'var(--text-muted)', margin: '4px 0 0' }}>
            Qualité logicielle et Explicabilité de l'IA (XAI).
          </p>
        </div>

        {/* Tabs simples */}
        <div
          style={{
            display: 'flex',
            gap: '10px',
            backgroundColor: 'var(--bg-panel)',
            padding: '4px',
            borderRadius: 'var(--radius-md)',
          }}
        >
          <button
            onClick={() => setActiveTab('qa')}
            style={{
              padding: '6px 16px',
              borderRadius: 'var(--radius-sm)',
              border: 'none',
              background: activeTab === 'qa' ? 'var(--bg-app)' : 'transparent',
              color: activeTab === 'qa' ? 'var(--text-main)' : 'var(--text-muted)',
              cursor: 'pointer',
              fontWeight: 'bold',
            }}
          >
            Quality (QA)
          </button>
          <button
            onClick={() => setActiveTab('xai')}
            style={{
              padding: '6px 16px',
              borderRadius: 'var(--radius-sm)',
              border: 'none',
              background: activeTab === 'xai' ? 'var(--bg-app)' : 'transparent',
              color: activeTab === 'xai' ? 'var(--text-main)' : 'var(--text-muted)',
              cursor: 'pointer',
              fontWeight: 'bold',
            }}
          >
            Explainability (XAI)
          </button>
        </div>
      </header>

      <div
        style={{
          backgroundColor: 'var(--bg-panel)',
          border: '1px solid var(--border-color)',
          borderRadius: 'var(--radius-lg)',
          padding: 'var(--spacing-6)',
          minHeight: '400px',
        }}
      >
        {activeTab === 'qa' ? (
          <div>
            <h3>Rapport de Qualité</h3>
            <div
              style={{
                display: 'grid',
                gap: 'var(--spacing-4)',
                gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))',
              }}
            >
              <MetricCard label="Code Coverage" value="94.2%" color="var(--color-success)" />
              <MetricCard label="Cyclomatic Complexity" value="4.5" color="var(--color-info)" />
              <MetricCard label="Technical Debt" value="2h 15m" color="var(--color-warning)" />
              <MetricCard label="Security Hotspots" value="0" color="var(--color-success)" />
            </div>
          </div>
        ) : (
          <div>
            <h3>Explicabilité (XAI)</h3>
            <p style={{ color: 'var(--text-muted)', marginBottom: '20px' }}>
              Justification des décisions prises par les agents génératifs.
            </p>
            <div
              style={{
                padding: '15px',
                borderLeft: '4px solid var(--color-primary)',
                backgroundColor: 'var(--bg-app)',
              }}
            >
              <strong>Dernière décision :</strong> Génération du composant <em>FlightController</em>
              .<br />
              <span style={{ fontSize: '0.9em', color: 'var(--text-muted)' }}>
                Raison : Le modèle fonctionnel SA indique une fonction critique "Stabilisation". Le
                pattern architectural "Control Loop" a été sélectionné avec une confiance de 98%
                basé sur les contraintes de latence.
              </span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function MetricCard({ label, value, color }: any) {
  return (
    <div
      style={{
        padding: '15px',
        backgroundColor: 'var(--bg-app)',
        borderRadius: 'var(--radius-md)',
        borderTop: `4px solid ${color}`,
      }}
    >
      <div style={{ fontSize: '0.8rem', color: 'var(--text-muted)', textTransform: 'uppercase' }}>
        {label}
      </div>
      <div style={{ fontSize: '1.8rem', fontWeight: 'bold', color: 'var(--text-main)' }}>
        {value}
      </div>
    </div>
  );
}
