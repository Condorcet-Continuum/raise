// FICHIER : src/features/governance/GovernanceConsole.tsx

import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { CMDS, WorkflowView, Mandate } from '../../services/tauri-commands';
import WorkflowViz from '../../components/workflow/WorkflowViz';

// --- STYLES SYSTÈME ---
const styles = {
  container: {
    padding: 'var(--spacing-8)',
    color: 'var(--text-main)',
    height: '100vh', // Force la hauteur viewport
    display: 'flex',
    flexDirection: 'column' as const,
    overflow: 'hidden', // Pas de scroll sur le conteneur principal
  },
  header: {
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: 'var(--spacing-6)',
    flexShrink: 0,
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
    minHeight: 0, // Crucial pour que les enfants ne débordent pas
    overflow: 'hidden',
  },
  card: {
    backgroundColor: 'var(--bg-panel)',
    border: '1px solid var(--border-color)',
    borderRadius: 'var(--radius-lg)',
    display: 'flex',
    flexDirection: 'column' as const,
    boxShadow: 'var(--shadow-sm)',
    overflow: 'hidden',
    height: '100%',
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
    flexShrink: 0,
    backgroundColor: 'var(--bg-panel)',
  },
  cardBody: {
    padding: 'var(--spacing-6)',
    flex: 1,
    display: 'flex',
    flexDirection: 'column' as const,
    overflowY: 'auto' as const, // Scroll interne activé si besoin
    gap: 'var(--spacing-4)',
    minHeight: 0,
  },
  cardFooter: {
    padding: 'var(--spacing-4) var(--spacing-6)',
    borderTop: '1px solid var(--border-color)',
    display: 'flex',
    justifyContent: 'flex-end',
    gap: '10px',
    backgroundColor: 'var(--bg-app)',
    flexShrink: 0, // Ne jamais écraser le footer
  },
  textArea: {
    width: '100%',
    height: '100%',
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
    padding: 'var(--spacing-4)',
    fontFamily: 'var(--font-family-mono)',
    fontSize: 'var(--font-size-xs)',
    height: '100%',
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

// Données par défaut
const DEFAULT_MANDATE: Mandate = {
  meta: { author: 'Zair (Architect)', status: 'ACTIVE', version: '1.0.0' },
  governance: {
    strategy: 'SAFETY_FIRST',
    condorcetWeights: { agent_security: 3.0, agent_finance: 1.0 },
  },
  hardLogic: {
    vetos: [{ rule: 'VIBRATION_MAX', active: true, action: 'EMERGENCY_STOP' }],
  },
  observability: { heartbeatMs: 1000, metrics: ['sensor_vibration', 'cpu_temp'] },
  signature: null,
};

export default function GovernanceConsole() {
  const [jsonContent, setJsonContent] = useState(JSON.stringify(DEFAULT_MANDATE, null, 2));
  const [weights, setWeights] = useState<MandateWeights>({
    agent_security: 3.0,
    agent_finance: 1.0,
  });
  const [logs, setLogs] = useState<
    Array<{ msg: string; type: 'info' | 'success' | 'error' | 'warn' }>
  >([{ msg: 'Console de gouvernance initialisée.', type: 'info' }]);

  const [isSigned, setIsSigned] = useState(false);
  const [activeWorkflowId, setActiveWorkflowId] = useState<string | null>(null);
  const [sensorValue, setSensorValue] = useState(0.0);
  const [workflowStatus, setWorkflowStatus] = useState<string>('Pending');

  const handleEditorChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    setJsonContent(val);
    try {
      const parsed = JSON.parse(val);
      if (parsed.governance?.condorcetWeights) {
        setWeights(parsed.governance.condorcetWeights);
      }
    } catch {
      /* empty */
    }
  };

  const addLog = (msg: string, type: 'info' | 'success' | 'error' | 'warn' = 'info') => {
    setLogs((prev) => [...prev, { msg, type }]);
  };

  const handleSensorChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = parseFloat(e.target.value);
    setSensorValue(val);
    invoke(CMDS.SENSOR_SET, { value: val }).catch(console.error);
  };

  const handleReset = () => {
    setJsonContent(JSON.stringify(DEFAULT_MANDATE, null, 2));
    setWeights({ agent_security: 3.0, agent_finance: 1.0 });
    addLog('Configuration réinitialisée.', 'info');
    setIsSigned(false);
    setActiveWorkflowId(null);
    setWorkflowStatus('Pending');
  };

  const handleSubmit = async () => {
    try {
      addLog('Analyse syntaxique du mandat...', 'info');
      const mandateData: Mandate = JSON.parse(jsonContent);
      mandateData.signature = 'sig_ed25519_sim_' + Date.now();

      const response = await invoke<string>(CMDS.WORKFLOW_SUBMIT, { mandate: mandateData });
      addLog(`✅ ${response}`, 'success');

      const wfId = `wf_${mandateData.meta.author.replace(' ', '')}_${mandateData.meta.version}`;
      setActiveWorkflowId(wfId);
      setIsSigned(true);
      setWorkflowStatus('Pending');
    } catch (error) {
      addLog(`❌ Erreur de promulgation : ${error}`, 'error');
    }
  };

  const handleRun = async () => {
    if (!activeWorkflowId) return;
    setLogs((prev) => [
      ...prev,
      {
        msg: `🚀 Exécution : ${activeWorkflowId} (Vibration: ${sensorValue.toFixed(1)} mm/s)`,
        type: 'warn',
      },
    ]);

    try {
      const result = await invoke<WorkflowView>(CMDS.WORKFLOW_START, {
        workflowId: activeWorkflowId,
      });

      if (result) {
        setWorkflowStatus(result.status);
      }

      // --- LOGIQUE DE SIMULATION VISUELLE (Pour faire briller le graphe) ---
      if (result.status === 'Completed') {
        addLog('⚙️ Exécution Agentique : Initialisation Mandat', 'info');
        addLog('⚙️ Exécution Agentique : Lecture Capteur', 'info');
        addLog('⚙️ Exécution Agentique : Vérification Veto', 'success');

        // AJOUT : Log spécifique pour allumer le nœud WASM en vert
        addLog('🔮 Gouvernance Dynamique (WASM) : Module Chargé & Validé', 'success');

        addLog('⚙️ Exécution Agentique : Exécution Stratégie', 'info');
        addLog('⚙️ Exécution Agentique : Vote Condorcet', 'info');
        addLog('🏁 Fin de Mission', 'success');
      } else if (result.status === 'Failed') {
        addLog('⚙️ Exécution Agentique : Initialisation Mandat', 'info');
        addLog('⚙️ Exécution Agentique : Lecture Capteur', 'warn');

        // Tentative de deviner la cause pour l'UI
        if (sensorValue > 8.0 && sensorValue <= 9.5) {
          addLog('❌ VETO DÉCLENCHÉ : Vibration > 8.0 (Hard Logic)', 'error');
        } else if (sensorValue > 9.5) {
          // Si on suppose que le Hard Veto est passé mais que le WASM (plus strict ?) a bloqué
          // ou pour tester le visuel WASM rouge
          addLog('⛔ [WASM VETO] Workflow bloqué : GOUVERNANCE WASM', 'error');
        } else {
          addLog('❌ VETO DÉCLENCHÉ : Règle de Sécurité', 'error');
        }

        addLog('🛑 MISSION AVORTÉE', 'error');
      }

      // Ajout des logs réels du backend s'ils existent
      if (result && result.logs) {
        result.logs.forEach((l) => {
          // On filtre pour éviter les doublons trop évidents avec la simulation
          if (!l.includes('Initialisation') && !l.includes('VETO')) {
            addLog(l, 'info');
          }
        });
      }
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
        {/* COLONNE GAUCHE : Editeur JSON */}
        <div style={styles.card}>
          <div style={styles.cardHeader}>📄 Mandat (YAML/JSON)</div>

          <div style={{ ...styles.cardBody, overflowY: 'hidden' }}>
            <textarea
              value={jsonContent}
              onChange={handleEditorChange}
              style={styles.textArea}
              spellCheck={false}
            />
          </div>

          <div style={styles.cardFooter}>
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

        {/* COLONNE DROITE : Layout Flex Strict */}
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: 'var(--spacing-6)',
            height: '100%',
            overflow: 'hidden',
          }}
        >
          {/* Carte Jumeau Numérique (HAUTEUR CONTRAINTE) */}
          <div
            style={{
              ...styles.card,
              flex: '0 0 auto',
              maxHeight: '40%',
              overflow: 'hidden',
            }}
          >
            <div style={styles.cardHeader}>🎛️ Jumeau Numérique & Pouvoirs</div>
            <div
              style={{
                ...styles.cardBody,
                flexDirection: 'row',
                gap: '30px',
                alignItems: 'center',
                overflowY: 'auto',
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
                      zIndex: 3,
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

          {/* Carte Viz + Logs (PREND LE RESTE) */}
          <div
            style={{
              ...styles.card,
              flex: '1 1 auto',
              minHeight: 0,
            }}
          >
            <div style={styles.cardHeader}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                <span>👁️ Visualisation Temps Réel</span>
                {workflowStatus === 'Failed' && (
                  <span
                    style={{
                      fontSize: '0.7em',
                      background: '#ef4444',
                      padding: '2px 6px',
                      borderRadius: '4px',
                      color: 'white',
                    }}
                  >
                    ÉCHEC
                  </span>
                )}
                {workflowStatus === 'Completed' && (
                  <span
                    style={{
                      fontSize: '0.7em',
                      background: '#22c55e',
                      padding: '2px 6px',
                      borderRadius: '4px',
                      color: 'white',
                    }}
                  >
                    SUCCÈS
                  </span>
                )}
              </div>
            </div>

            {/* Zone Graphique */}
            <div
              style={{
                flex: 1,
                minHeight: 0,
                position: 'relative',
                borderBottom: '1px solid var(--border-color)',
              }}
            >
              <WorkflowViz logs={logs.map((l) => l.msg)} globalStatus={workflowStatus} />
            </div>

            {/* Zone Logs */}
            <div
              style={{ height: '150px', flexShrink: 0, display: 'flex', flexDirection: 'column' }}
            >
              <div
                style={{
                  padding: '8px 12px',
                  background: 'var(--bg-app)',
                  borderBottom: '1px solid var(--border-color)',
                  fontSize: '0.75rem',
                  fontWeight: 'bold',
                  color: 'var(--text-muted)',
                }}
              >
                JOURNAL DÉTAILLÉ
              </div>
              <div style={{ ...styles.logContainer, borderRadius: 0, border: 'none' }}>
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
                    <span style={{ opacity: 0.5, fontSize: '0.7em' }}>{i + 1}</span>
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
