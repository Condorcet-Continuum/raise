import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { CMDS } from '@/services/tauri-commands';

// --- STYLES SYSTÈME ---
const styles = {
  container: {
    padding: 'var(--spacing-8)',
    color: 'var(--text-main)',
    height: '100%',
    display: 'flex',
    flexDirection: 'column' as const,
    overflow: 'hidden',
  },
  header: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: 'var(--spacing-6)',
  },
  title: {
    fontSize: 'var(--font-size-3xl)',
    fontWeight: 'var(--font-weight-bold)',
    marginBottom: 'var(--spacing-1)',
    color: 'var(--text-main)',
  },
  subtitle: {
    color: 'var(--text-muted)',
    fontSize: 'var(--font-size-sm)',
  },
  grid: {
    display: 'grid',
    gridTemplateColumns: 'minmax(0, 7fr) minmax(0, 5fr)',
    gap: 'var(--spacing-6)',
    flex: 1,
    minHeight: 0,
  },
  card: {
    backgroundColor: 'var(--bg-panel)',
    border: '1px solid var(--border-color)',
    borderRadius: 'var(--radius-lg)',
    display: 'flex',
    flexDirection: 'column' as const,
    boxShadow: 'var(--shadow-sm)',
    overflow: 'hidden',
  },
  cardHeader: {
    padding: 'var(--spacing-4) var(--spacing-6)',
    borderBottom: '1px solid var(--border-color)',
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    fontWeight: 'var(--font-weight-bold)',
    fontSize: 'var(--font-size-lg)',
    color: 'var(--text-main)',
  },
  cardBody: {
    padding: 'var(--spacing-6)',
    flex: 1,
    display: 'flex',
    flexDirection: 'column' as const,
    overflowY: 'auto' as const,
    gap: 'var(--spacing-4)',
  },
  textArea: {
    width: '100%',
    flex: 1,
    backgroundColor: 'var(--bg-app)',
    border: '1px solid var(--border-color)',
    borderRadius: 'var(--radius-md)',
    color: 'var(--text-main)',
    fontFamily: 'var(--font-family-mono)',
    fontSize: 'var(--font-size-sm)',
    padding: 'var(--spacing-4)',
    resize: 'none' as const,
    outline: 'none',
    lineHeight: 'var(--line-height-relaxed)',
  },
  logContainer: {
    backgroundColor: 'var(--bg-app)',
    borderRadius: 'var(--radius-md)',
    border: '1px solid var(--border-color)',
    padding: 'var(--spacing-4)',
    fontFamily: 'var(--font-family-mono)',
    fontSize: 'var(--font-size-xs)',
    flex: 1,
    overflowY: 'auto' as const,
  },
  button: (primary = false, danger = false) => ({
    backgroundColor: danger
      ? 'var(--color-error)'
      : primary
      ? 'var(--color-primary)'
      : 'transparent',
    color: primary || danger ? '#ffffff' : 'var(--color-primary)',
    border: primary || danger ? 'none' : '1px solid var(--color-primary)',
    padding: '8px 20px',
    borderRadius: 'var(--radius-sm)',
    cursor: 'pointer',
    fontWeight: 'var(--font-weight-medium)',
    fontSize: 'var(--font-size-sm)',
    display: 'flex',
    alignItems: 'center',
    gap: '8px',
    transition: 'all 0.2s',
    opacity: 1,
  }),
  badge: {
    padding: '4px 12px',
    backgroundColor: 'var(--bg-panel)',
    border: '1px solid var(--color-success)',
    color: 'var(--color-success)',
    borderRadius: 'var(--radius-full)',
    fontSize: 'var(--font-size-xs)',
    fontWeight: 'var(--font-weight-medium)',
    display: 'flex',
    alignItems: 'center',
    gap: '6px',
  },
  sliderWrapper: {
    display: 'flex',
    flexDirection: 'column' as const,
    alignItems: 'center',
    gap: 'var(--spacing-2)',
    padding: 'var(--spacing-2)',
    backgroundColor: 'var(--bg-app)',
    borderRadius: 'var(--radius-md)',
    border: '1px solid var(--border-color)',
  },
  sliderContainer: {
    height: '180px',
    width: '40px',
    position: 'relative' as const,
    display: 'flex',
    justifyContent: 'center',
    backgroundColor: 'rgba(0,0,0,0.1)',
    borderRadius: 'var(--radius-full)',
    padding: '4px',
  },
  sliderInput: {
    WebkitAppearance: 'slider-vertical' as const,
    width: '20px',
    height: '100%',
    cursor: 'pointer',
    zIndex: 2,
    outline: 'none',
  },
};

// --- TYPES ---
interface MandateWeights {
  agent_security: number;
  agent_finance: number;
}

interface WorkflowExecutionResult {
  id: string;
  status: 'Pending' | 'Running' | 'Completed' | 'Failed' | 'Paused';
  logs: string[];
}

const DEFAULT_MANDATE = {
  meta: { author: 'Zair (Architect)', status: 'DRAFT', version: '1.0.0' },
  governance: {
    strategy: 'SAFETY_CRITICAL',
    condorcet_weights: { agent_security: 3.0, agent_finance: 1.0 },
  },
  hard_logic: {
    vetos: [{ rule: 'VIBRATION_MAX', active: true, action: 'EMERGENCY_SHUTDOWN' }],
  },
  observability: { heartbeat_ms: 1000, metrics: ['sensor_vibration', 'cpu_temp'] },
  signature: null as string | null,
};

export default function GovernanceConsole() {
  const [jsonContent, setJsonContent] = useState(JSON.stringify(DEFAULT_MANDATE, null, 2));
  const [weights, setWeights] = useState<MandateWeights>(
    DEFAULT_MANDATE.governance.condorcet_weights,
  );
  const [logs, setLogs] = useState<
    Array<{ msg: string; type: 'info' | 'success' | 'error' | 'warn' }>
  >([{ msg: 'Console de gouvernance initialisée.', type: 'info' }]);
  const [isSigned, setIsSigned] = useState(false);
  const [activeWorkflowId, setActiveWorkflowId] = useState<string | null>(null);
  const [sensorValue, setSensorValue] = useState(0.0);

  const handleEditorChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    setJsonContent(val);
    try {
      const parsed = JSON.parse(val);
      if (parsed.governance?.condorcet_weights) {
        setWeights(parsed.governance.condorcet_weights);
      }
    } catch {
      // Ignorer les erreurs JSON pendant la frappe
    }
  };

  const addLog = (msg: string, type: 'info' | 'success' | 'error' | 'warn' = 'info') => {
    setLogs((prev) => [...prev, { msg, type }]);
  };

  const handleSensorChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = parseFloat(e.target.value);
    setSensorValue(val);
    invoke('set_sensor_value', { value: val }).catch(console.error);
  };

  const handleReset = () => {
    setJsonContent(JSON.stringify(DEFAULT_MANDATE, null, 2));
    setWeights(DEFAULT_MANDATE.governance.condorcet_weights);
    addLog('Configuration réinitialisée.', 'info');
    setIsSigned(false);
    setActiveWorkflowId(null);
  };

  const handleSubmit = async () => {
    try {
      addLog('Analyse syntaxique du mandat...', 'info');
      const mandateData = JSON.parse(jsonContent);
      mandateData.signature = 'sig_ed25519_sim_' + Date.now();

      // Appel à Rust sans stocker la réponse inutilisée pour satisfaire ESLint
      await invoke<string>(CMDS.WORKFLOW_SUBMIT, { mandate: mandateData });
      addLog('✅ MANDAT PROMULGUÉ AVEC SUCCÈS', 'success');

      const wfId = `wf_${mandateData.meta.author.replace(' ', '')}_${mandateData.meta.version}`;
      setActiveWorkflowId(wfId);
      setIsSigned(true);
    } catch (error) {
      addLog(`❌ Erreur de promulgation : ${error}`, 'error');
    }
  };

  const handleRun = async () => {
    if (!activeWorkflowId) return;
    setLogs([
      { msg: `🚀 Exécution : ${activeWorkflowId} (Vibration: ${sensorValue} mm/s)`, type: 'warn' },
    ]);
    try {
      const result = await invoke<WorkflowExecutionResult>(CMDS.WORKFLOW_START, {
        workflowId: activeWorkflowId,
      });
      if (result && result.logs) {
        result.logs.forEach((l) => {
          if (l.includes('VETO')) addLog(l, 'error');
          else if (l.includes('✅')) addLog(l, 'success');
          else addLog(l, 'info');
        });
      }
      if (result.status === 'Failed') addLog("🛑 ARRÊT D'URGENCE DÉCLENCHÉ", 'error');
      else addLog('🏁 Fin de mission : Workflow complété', 'success');
    } catch (error) {
      addLog(`❌ Erreur exécution : ${error}`, 'error');
    }
  };

  const total = (weights.agent_security || 0) + (weights.agent_finance || 0) || 1;
  const secPercent = ((weights.agent_security || 0) / total) * 100;
  const finPercent = ((weights.agent_finance || 0) / total) * 100;

  return (
    <div style={styles.container}>
      <div style={styles.header}>
        <div>
          <h1 style={styles.title}>🏛️ Gouvernance</h1>
          <p style={styles.subtitle}>Contrôle Neuro-Symbolique & Jumeau Numérique</p>
        </div>
        <div style={styles.badge}>
          <span
            style={{
              width: 8,
              height: 8,
              backgroundColor: 'var(--color-success)',
              borderRadius: '50%',
            }}
          ></span>
          MOTEUR ACTIF
        </div>
      </div>

      <div style={styles.grid}>
        <div style={styles.card}>
          <div style={styles.cardHeader}>📄 Mandat (YAML/JSON)</div>
          <div style={styles.cardBody}>
            <textarea
              value={jsonContent}
              onChange={handleEditorChange}
              style={styles.textArea}
              spellCheck={false}
            />
            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px' }}>
              <button
                onClick={handleReset}
                style={{ ...styles.button(false), padding: '4px 12px', fontSize: '0.75rem' }}
              >
                ↺ Restaurer
              </button>
              <button onClick={handleSubmit} style={styles.button(!isSigned && !activeWorkflowId)}>
                ✍️ Promulguer
              </button>
              {activeWorkflowId && (
                <button onClick={handleRun} style={styles.button(false, true)}>
                  🚀 Lancer
                </button>
              )}
            </div>
          </div>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-6)' }}>
          <div style={styles.card}>
            <div style={styles.cardHeader}>🎛️ Jumeau Numérique & Pouvoirs</div>
            <div
              style={{
                ...styles.cardBody,
                flexDirection: 'row',
                gap: '30px',
                alignItems: 'center',
              }}
            >
              <div style={styles.sliderWrapper}>
                <div style={styles.sliderContainer}>
                  <div
                    style={{
                      position: 'absolute',
                      bottom: '4px',
                      width: '12px',
                      height: `${(sensorValue / 15) * 100}%`,
                      backgroundColor:
                        sensorValue > 8.0 ? 'var(--color-error)' : 'var(--color-success)',
                      borderRadius: 'var(--radius-full)',
                      transition: 'all 0.2s',
                      zIndex: 1,
                    }}
                  />
                  <div
                    style={{
                      position: 'absolute',
                      bottom: `${(8.0 / 15) * 100}%`,
                      width: '100%',
                      height: '2px',
                      backgroundColor: 'rgba(255,255,255,0.8)',
                      zIndex: 3, // Correction de z_index ici
                    }}
                  />
                  <input
                    type="range"
                    min="0"
                    max="15"
                    step="0.1"
                    value={sensorValue}
                    onChange={handleSensorChange}
                    style={styles.sliderInput}
                  />
                </div>
                <div style={{ textAlign: 'center' }}>
                  <div style={{ fontSize: '0.65rem', color: 'var(--text-muted)' }}>VIBRATION</div>
                  <div
                    style={{
                      fontWeight: 'bold',
                      color: sensorValue > 8.0 ? 'var(--color-error)' : 'inherit',
                    }}
                  >
                    {sensorValue.toFixed(1)}
                  </div>
                </div>
              </div>

              <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '15px' }}>
                <div>
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      fontSize: '0.8rem',
                      marginBottom: '4px',
                    }}
                  >
                    <strong style={{ color: 'var(--color-primary)' }}>🛡️ SÉCU</strong>
                    <span>{weights.agent_security}x</span>
                  </div>
                  <div
                    style={{
                      width: '100%',
                      height: '8px',
                      backgroundColor: 'var(--bg-app)',
                      borderRadius: 'var(--radius-full)',
                      overflow: 'hidden',
                    }}
                  >
                    <div
                      style={{
                        width: `${secPercent}%`,
                        height: '100%',
                        backgroundColor: 'var(--color-primary)',
                      }}
                    ></div>
                  </div>
                </div>
                <div>
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      fontSize: '0.8rem',
                      marginBottom: '4px',
                    }}
                  >
                    <strong style={{ color: 'var(--color-success)' }}>💰 FIN</strong>
                    <span>{weights.agent_finance}x</span>
                  </div>
                  <div
                    style={{
                      width: '100%',
                      height: '8px',
                      backgroundColor: 'var(--bg-app)',
                      borderRadius: 'var(--radius-full)',
                      overflow: 'hidden',
                    }}
                  >
                    <div
                      style={{
                        width: `${finPercent}%`,
                        height: '100%',
                        backgroundColor: 'var(--color-success)',
                      }}
                    ></div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div style={{ ...styles.card, flex: 1 }}>
            <div style={styles.cardHeader}>📟 Journal</div>
            <div style={{ ...styles.cardBody, padding: 0 }}>
              <div style={styles.logContainer}>
                {logs.map((log, i) => (
                  <div
                    key={i}
                    style={{
                      marginBottom: '4px',
                      color:
                        log.type === 'error'
                          ? 'var(--color-error)'
                          : log.type === 'success'
                          ? 'var(--color-success)'
                          : 'var(--text-muted)',
                      display: 'flex',
                      gap: '8px',
                    }}
                  >
                    <span>&gt;</span>
                    <span>{log.msg}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
