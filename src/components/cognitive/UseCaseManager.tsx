import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

interface LogEntry {
  timestamp: string;
  source: 'SYSTEM' | 'PLUGIN' | 'ERROR';
  message: string;
}

export default function UseCaseManager() {
  const [plugins, setPlugins] = useState<string[]>([]);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [loading, setLoading] = useState(false);

  // 1. On utilise useCallback pour stabiliser cette fonction
  // Elle ne sera pas recréée à chaque rendu
  const addLog = useCallback((source: LogEntry['source'], message: string) => {
    const time = new Date().toLocaleTimeString();
    setLogs((prev) => [...prev, { timestamp: time, source, message }]);
  }, []);

  // 2. Idem pour refreshPluginList, qui dépend maintenant de addLog
  const refreshPluginList = useCallback(async () => {
    try {
      const list = await invoke<string[]>('cognitive_list_plugins');
      setPlugins(list);
    } catch (e) {
      addLog('ERROR', `Impossible de lister les plugins : ${e}`);
    }
  }, [addLog]);

  // 3. Le useEffect inclut maintenant ses dépendances sans risque de boucle infinie
  useEffect(() => {
    refreshPluginList();
    addLog('SYSTEM', 'Initialisation du Use-Case Manager...');
  }, [refreshPluginList, addLog]);

  const handleLoadPlugin = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'WebAssembly', extensions: ['wasm'] }],
      });

      if (selected && typeof selected === 'string') {
        setLoading(true);
        const path = selected;
        const fileName = path.split(/[\\/]/).pop() || 'unknown';
        const pluginId = fileName.replace('.wasm', '');

        addLog('SYSTEM', `Chargement de : ${fileName}...`);

        await invoke('cognitive_load_plugin', {
          pluginId: pluginId,
          wasmPath: path,
          space: 'un2',
          db: 'default',
        });

        addLog('SYSTEM', `✅ Plugin '${pluginId}' prêt.`);
        await refreshPluginList();
      }
    } catch (e) {
      addLog('ERROR', `Échec du chargement : ${e}`);
    } finally {
      setLoading(false);
    }
  };

  const handleRunPlugin = async (pluginId: string) => {
    addLog('SYSTEM', `▶️ Exécution de : ${pluginId}`);
    try {
      const result = await invoke<number>('cognitive_run_plugin', { pluginId });
      if (result === 1) {
        addLog('PLUGIN', `Succès (Code: ${result})`);
      } else {
        addLog('PLUGIN', `⚠️ Code retour : ${result}`);
      }
    } catch (e) {
      addLog('ERROR', `Erreur d'exécution : ${e}`);
    }
  };

  // --- STYLES ---
  const panelStyle: React.CSSProperties = {
    backgroundColor: 'var(--bg-panel)',
    border: '1px solid var(--border-color)',
    borderRadius: 'var(--radius-lg)',
    padding: 'var(--spacing-4)',
    height: '100%',
    display: 'flex',
    flexDirection: 'column',
    overflow: 'hidden',
  };

  const buttonStyle: React.CSSProperties = {
    padding: '8px 16px',
    backgroundColor: 'var(--color-primary)',
    color: '#fff',
    border: 'none',
    borderRadius: 'var(--radius-md)',
    cursor: 'pointer',
    fontWeight: 500,
    opacity: loading ? 0.7 : 1,
  };

  return (
    <div
      style={{
        display: 'flex',
        gap: 'var(--spacing-4)',
        height: '100%',
        padding: 'var(--spacing-4)',
      }}
    >
      {/* COLONNE GAUCHE : LISTE */}
      <div style={{ flex: 1, ...panelStyle }}>
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            marginBottom: 'var(--spacing-4)',
          }}
        >
          <h3 style={{ margin: 0, color: 'var(--text-main)' }}>Plugins Chargés</h3>
          <button onClick={handleLoadPlugin} disabled={loading} style={buttonStyle}>
            {loading ? '...' : '🔌 Charger .wasm'}
          </button>
        </div>

        <div
          style={{
            flex: 1,
            overflowY: 'auto',
            display: 'flex',
            flexDirection: 'column',
            gap: '8px',
          }}
        >
          {plugins.length === 0 ? (
            <div style={{ color: 'var(--text-muted)', textAlign: 'center', marginTop: '20px' }}>
              Aucun plugin actif.
            </div>
          ) : (
            plugins.map((id) => (
              <div
                key={id}
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  padding: '12px',
                  backgroundColor: 'var(--bg-app)',
                  border: '1px solid var(--border-color)',
                  borderRadius: 'var(--radius-md)',
                }}
              >
                <span style={{ fontWeight: 'bold', color: 'var(--text-main)' }}>🧩 {id}</span>
                <button
                  onClick={() => handleRunPlugin(id)}
                  style={{
                    padding: '4px 12px',
                    border: '1px solid var(--color-primary)',
                    background: 'transparent',
                    color: 'var(--color-primary)',
                    borderRadius: 'var(--radius-sm)',
                    cursor: 'pointer',
                  }}
                >
                  Exécuter
                </button>
              </div>
            ))
          )}
        </div>
      </div>

      {/* COLONNE DROITE : LOGS */}
      <div style={{ flex: 2, ...panelStyle, backgroundColor: '#1e1e1e', color: '#d4d4d4' }}>
        <div
          style={{
            borderBottom: '1px solid #333',
            paddingBottom: '8px',
            marginBottom: '8px',
            display: 'flex',
            justifyContent: 'space-between',
          }}
        >
          <span style={{ fontSize: '0.8rem', textTransform: 'uppercase', color: '#888' }}>
            Terminal de Sortie
          </span>
          <button
            onClick={() => setLogs([])}
            style={{
              background: 'none',
              border: 'none',
              color: '#888',
              cursor: 'pointer',
              fontSize: '0.8rem',
            }}
          >
            Effacer
          </button>
        </div>

        <div
          style={{
            flex: 1,
            overflowY: 'auto',
            fontFamily: 'monospace',
            fontSize: '0.9rem',
            display: 'flex',
            flexDirection: 'column',
            gap: '4px',
          }}
        >
          {logs.map((log, i) => (
            <div key={i} style={{ display: 'flex', gap: '10px' }}>
              <span style={{ color: '#555' }}>[{log.timestamp}]</span>
              <span
                style={{
                  fontWeight: 'bold',
                  color:
                    log.source === 'ERROR'
                      ? '#f87171'
                      : log.source === 'PLUGIN'
                      ? '#4ade80'
                      : '#60a5fa',
                  width: '60px',
                }}
              >
                {log.source}
              </span>
              <span style={{ wordBreak: 'break-all' }}>{log.message}</span>
            </div>
          ))}
          {logs.length === 0 && <span style={{ color: '#444' }}>Prêt...</span>}
        </div>
      </div>
    </div>
  );
}
