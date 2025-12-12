import { useState, useEffect } from 'react';

interface LogEntry {
  id: number;
  timestamp: string;
  message: string;
  type: 'info' | 'error' | 'success';
}

export function ExecutionMonitor() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [isRunning, setIsRunning] = useState(false);

  // Simulation de logs
  const runWorkflow = () => {
    setIsRunning(true);
    setLogs([]);

    const steps = [
      { msg: 'Initialisation du workflow...', type: 'info' },
      { msg: "Chargement des variables d'environnement...", type: 'info' },
      { msg: 'Exécution du script : build_project.sh', type: 'info' },
      { msg: 'Compilation réussie (Rust 1.75)', type: 'success' },
      { msg: 'Déploiement sur cluster K8s...', type: 'info' },
      { msg: 'Workflow terminé avec succès.', type: 'success' },
    ];

    steps.forEach((step, index) => {
      setTimeout(() => {
        setLogs((prev) => [
          ...prev,
          {
            id: Date.now(),
            timestamp: new Date().toLocaleTimeString(),
            message: step.msg,
            type: step.type as any,
          },
        ]);
        if (index === steps.length - 1) setIsRunning(false);
      }, (index + 1) * 800);
    });
  };

  return (
    <div
      style={{
        height: '200px',
        backgroundColor: 'var(--bg-panel)',
        borderTop: '1px solid var(--border-color)',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <header
        style={{
          padding: 'var(--spacing-2) var(--spacing-4)',
          borderBottom: '1px solid var(--border-color)',
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          backgroundColor: 'var(--color-gray-50)',
        }}
      >
        <span
          style={{
            fontSize: 'var(--font-size-sm)',
            fontWeight: 'bold',
            color: 'var(--text-muted)',
          }}
        >
          CONSOLE D'EXÉCUTION
        </span>
        <button
          onClick={runWorkflow}
          disabled={isRunning}
          style={{
            padding: '4px 12px',
            backgroundColor: isRunning ? 'var(--color-gray-400)' : 'var(--color-success)',
            color: '#fff',
            border: 'none',
            borderRadius: 'var(--radius-sm)',
            fontSize: 'var(--font-size-xs)',
            cursor: isRunning ? 'wait' : 'pointer',
            fontWeight: 'bold',
          }}
        >
          {isRunning ? 'Exécution...' : '▶ Lancer'}
        </button>
      </header>

      <div
        style={{
          flex: 1,
          overflowY: 'auto',
          padding: 'var(--spacing-2)',
          fontFamily: 'var(--font-family-mono)',
          fontSize: 'var(--font-size-xs)',
          backgroundColor: 'var(--bg-app)', // Fond sombre en dark mode pour l'aspect terminal
        }}
      >
        {logs.length === 0 && !isRunning && (
          <div style={{ color: 'var(--text-muted)', padding: 'var(--spacing-2)' }}>Prêt.</div>
        )}

        {logs.map((log) => (
          <div key={log.id} style={{ marginBottom: '4px', display: 'flex', gap: '10px' }}>
            <span style={{ color: 'var(--text-muted)' }}>[{log.timestamp}]</span>
            <span
              style={{
                color:
                  log.type === 'error'
                    ? 'var(--color-error)'
                    : log.type === 'success'
                    ? 'var(--color-success)'
                    : 'var(--text-main)',
              }}
            >
              {log.message}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
