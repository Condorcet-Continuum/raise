import { useState, useMemo } from 'react';
import { useGenetics } from '@/hooks/useGenetics';
import ArchitectureViewer from './ArchitectureViewer';
import { OptimizationRequest } from '@/services/geneticsService';

export default function GeneticsDashboard() {
  const { runOptimization, loading, progress, history, result, canRun, stats } = useGenetics();

  const [useDemo, setUseDemo] = useState(false);
  const [params, setParams] = useState({
    population_size: 100,
    max_generations: 50,
    mutation_rate: 0.1,
    crossover_rate: 0.8,
  });
  const [selectedIdx, setSelectedIdx] = useState<number | null>(null);

  // CONTEXTE : Flux et Charges pour le Viewer
  const [lastContext, setLastContext] = useState<{
    flows: { source: string; target: string; volume: number }[];
    loads: Record<string, number>;
    caps: Record<string, number>;
  } | null>(null);

  // --- GÉNÉRATEUR DE DONNÉES LOCAL ---
  const generateDemoData = (): OptimizationRequest => {
    const funcs = Array.from({ length: 25 }, (_, i) => ({
      id: `F${i}`,
      load: Math.floor(Math.random() * 20) + 5,
    }));
    const comps = Array.from({ length: 5 }, (_, i) => ({ id: `CPU_${i}`, capacity: 100 }));
    const flows = Array.from({ length: 40 }, () => ({
      source_id: `F${Math.floor(Math.random() * 25)}`,
      target_id: `F${Math.floor(Math.random() * 25)}`,
      volume: Math.floor(Math.random() * 50) + 10,
    })).filter((f) => f.source_id !== f.target_id);

    return { ...params, functions: funcs, components: comps, flows: flows };
  };

  const handleRun = async () => {
    let requestData: OptimizationRequest | undefined;

    if (useDemo) {
      const demoData = generateDemoData();
      requestData = demoData;

      const loadsMap: Record<string, number> = {};
      demoData.functions.forEach((f) => (loadsMap[f.id] = f.load));
      const capsMap: Record<string, number> = {};
      demoData.components.forEach((c) => (capsMap[c.id] = c.capacity));

      setLastContext({
        flows: demoData.flows.map((f) => ({
          source: f.source_id,
          target: f.target_id,
          volume: f.volume,
        })),
        loads: loadsMap,
        caps: capsMap,
      });
    } else {
      // En mode réel, on initialise vide pour l'instant (TODO: connecter au store)
      setLastContext({ flows: [], loads: {}, caps: {} });
    }

    await runOptimization(params, requestData);
  };

  // Styles CSS
  const styles: Record<string, React.CSSProperties> = {
    container: {
      padding: '20px',
      backgroundColor: 'var(--bg-app)',
      color: 'var(--text-main)',
      height: '100%',
      display: 'flex',
      flexDirection: 'column',
    },
    header: {
      marginBottom: '20px',
      borderBottom: '1px solid var(--border-color)',
      paddingBottom: '15px',
      flexShrink: 0,
    },
    grid: {
      display: 'grid',
      gridTemplateColumns: '320px 1fr',
      gap: '20px',
      flex: 1,
      minHeight: 0,
      overflow: 'hidden',
    },
    leftPanel: {
      backgroundColor: 'var(--bg-panel)',
      padding: '20px',
      borderRadius: '12px',
      border: '1px solid var(--border-color)',
      overflowY: 'auto',
      display: 'flex',
      flexDirection: 'column',
      gap: '20px',
      maxHeight: '100%',
    },
    rightPanel: {
      backgroundColor: 'var(--bg-panel)',
      padding: '20px',
      borderRadius: '12px',
      border: '1px solid var(--border-color)',
      overflowY: 'auto',
      position: 'relative',
      maxHeight: '100%',
    },
    inputGroup: { display: 'flex', flexDirection: 'column', gap: '6px' },
    label: {
      display: 'flex',
      justifyContent: 'space-between',
      fontSize: '12px',
      color: 'var(--text-muted)',
    },
    val: { color: 'var(--color-accent)', fontWeight: 'bold' },
    range: { width: '100%', cursor: 'pointer', accentColor: 'var(--color-accent)' },
    demoToggle: {
      display: 'flex',
      alignItems: 'center',
      gap: '10px',
      fontSize: '12px',
      padding: '12px',
      backgroundColor: 'rgba(255,255,255,0.05)',
      borderRadius: '8px',
      cursor: 'pointer',
      border: '1px solid var(--border-color)',
    },
    btn: {
      width: '100%',
      padding: '14px',
      marginTop: 'auto',
      background: canRun || useDemo ? 'var(--color-accent)' : '#444',
      color: '#fff',
      border: 'none',
      borderRadius: '8px',
      cursor: canRun || useDemo ? 'pointer' : 'not-allowed',
      fontWeight: 'bold',
    },
    chartArea: {
      position: 'relative',
      height: '320px',
      borderLeft: '2px solid var(--text-muted)',
      borderBottom: '2px solid var(--text-muted)',
      margin: '20px 20px 50px 60px',
      backgroundColor: 'rgba(255,255,255,0.02)',
    },
    point: {
      position: 'absolute',
      width: '14px',
      height: '14px',
      borderRadius: '50%',
      cursor: 'pointer',
      transform: 'translate(-50%, 50%)',
      border: '2px solid #333',
      transition: 'all 0.3s',
    },
    convergence: {
      display: 'flex',
      alignItems: 'flex-end',
      gap: '2px',
      height: '60px',
      borderBottom: '1px solid #444',
      marginBottom: '30px',
      backgroundColor: 'rgba(0,0,0,0.2)',
      padding: '5px',
    },
    table: { width: '100%', borderCollapse: 'collapse', fontSize: '12px', marginTop: '10px' },
    th: {
      textAlign: 'left',
      padding: '8px',
      borderBottom: '1px solid #444',
      color: 'var(--text-muted)',
    },
    td: { padding: '8px', borderBottom: '1px solid #333' },
  };

  const maxCostHistory = useMemo(() => {
    if (history.length === 0) return 1;
    return Math.max(...history.map((h) => Math.abs(h.best_fitness[0])));
  }, [history]);

  const renderPareto = () => {
    if (!result || result.pareto_front.length === 0) {
      return (
        <div
          style={{
            ...styles.chartArea,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            color: 'var(--text-muted)',
          }}
        >
          {loading ? 'Calcul...' : 'En attente...'}
        </div>
      );
    }
    const costs = result.pareto_front.map((s) => ({
      c: Math.abs(s.fitness[0]),
      b: Math.abs(s.fitness[1]),
    }));
    const maxC = Math.max(...costs.map((x) => x.c), 1.0) * 1.1;
    const maxB = Math.max(...costs.map((x) => x.b), 1.0) * 1.1;

    return (
      <div style={styles.chartArea}>
        <div
          style={{
            position: 'absolute',
            top: '50%',
            left: '-60px',
            transform: 'rotate(-90deg) translateX(50%)',
            width: '300px',
            textAlign: 'center',
            fontSize: '11px',
            color: 'var(--text-muted)',
          }}
        >
          ↑ Déséquilibre
        </div>
        <div
          style={{
            position: 'absolute',
            bottom: '-35px',
            width: '100%',
            textAlign: 'center',
            fontSize: '11px',
            color: 'var(--text-muted)',
          }}
        >
          Couplage →
        </div>
        <div
          style={{
            position: 'absolute',
            left: 0,
            top: 0,
            width: '100%',
            height: '100%',
            backgroundImage:
              'linear-gradient(#444 1px, transparent 1px), linear-gradient(90deg, #444 1px, transparent 1px)',
            backgroundSize: '40px 40px',
            opacity: 0.1,
            pointerEvents: 'none',
          }}
        ></div>
        {costs.map((item, i) => {
          const xPos = (item.c / maxC) * 100;
          const yPos = (item.b / maxB) * 100;
          const isSel = selectedIdx === i;
          return (
            <div
              key={i}
              onClick={() => setSelectedIdx(i)}
              style={{
                ...styles.point,
                left: `${xPos}%`,
                bottom: `${yPos}%`,
                backgroundColor: isSel ? 'var(--color-accent)' : '#ef4444',
                zIndex: isSel ? 20 : 10,
                transform: `translate(-50%, 50%) scale(${isSel ? 1.5 : 1})`,
                boxShadow: isSel ? '0 0 15px var(--color-accent)' : 'none',
              }}
              title={`Sol ${i + 1}`}
            />
          );
        })}
      </div>
    );
  };

  return (
    <div style={styles.container}>
      <header style={styles.header}>
        <h2 style={{ color: 'var(--color-accent)', margin: 0 }}>Optimisation NSGA-II</h2>
        <p style={{ fontSize: '12px', opacity: 0.7, marginTop: '5px' }}>
          {useDemo
            ? 'Mode Simulation'
            : `${stats.functionsCount} Fonctions • ${stats.componentsCount} Composants`}
        </p>
      </header>
      <div style={styles.grid}>
        <div style={styles.leftPanel}>
          <label style={styles.demoToggle}>
            <input
              type="checkbox"
              checked={useDemo}
              onChange={(e) => setUseDemo(e.target.checked)}
            />{' '}
            <span>🧪 Mode Démo</span>
          </label>
          <hr
            style={{
              border: 'none',
              borderTop: '1px solid var(--border-color)',
              width: '100%',
              margin: '0',
            }}
          />
          <div style={styles.inputGroup}>
            <label style={styles.label}>
              Population <span style={styles.val}>{params.population_size}</span>
            </label>
            <input
              type="range"
              min="20"
              max="500"
              step="20"
              style={styles.range}
              value={params.population_size}
              onChange={(e) => setParams({ ...params, population_size: +e.target.value })}
            />
          </div>
          <div style={styles.inputGroup}>
            <label style={styles.label}>
              Générations <span style={styles.val}>{params.max_generations}</span>
            </label>
            <input
              type="range"
              min="10"
              max="200"
              step="10"
              style={styles.range}
              value={params.max_generations}
              onChange={(e) => setParams({ ...params, max_generations: +e.target.value })}
            />
          </div>
          <div style={styles.inputGroup}>
            <label style={styles.label}>
              Mutation <span style={styles.val}>{params.mutation_rate}</span>
            </label>
            <input
              type="range"
              min="0"
              max="0.5"
              step="0.01"
              style={styles.range}
              value={params.mutation_rate}
              onChange={(e) => setParams({ ...params, mutation_rate: +e.target.value })}
            />
          </div>
          <div style={styles.inputGroup}>
            <label style={styles.label}>
              Crossover <span style={styles.val}>{params.crossover_rate}</span>
            </label>
            <input
              type="range"
              min="0.5"
              max="1.0"
              step="0.05"
              style={styles.range}
              value={params.crossover_rate}
              onChange={(e) => setParams({ ...params, crossover_rate: +e.target.value })}
            />
          </div>

          <button
            style={styles.btn}
            onClick={handleRun}
            disabled={loading || (!canRun && !useDemo)}
          >
            {/* CORRECTION : Utilisation de la variable progress ici */}
            {loading ? `Calcul (Gen ${progress?.generation || 0})...` : 'Lancer'}
          </button>
        </div>

        <div style={styles.rightPanel}>
          <h4
            style={{
              margin: '0 0 15px 0',
              fontSize: '14px',
              borderBottom: '1px solid #333',
              paddingBottom: '10px',
            }}
          >
            Convergence & Front de Pareto
          </h4>
          <div style={styles.convergence}>
            {history.map((h, i) => {
              const val = Math.abs(h.best_fitness[0]);
              const hPerc = (val / maxCostHistory) * 100;
              return (
                <div
                  key={i}
                  style={{
                    flex: 1,
                    backgroundColor: 'var(--color-accent)',
                    opacity: 0.6,
                    height: `${Math.max(5, hPerc)}%`,
                    borderRadius: '2px 2px 0 0',
                  }}
                />
              );
            })}
          </div>
          {renderPareto()}

          {selectedIdx !== null && result && (
            <div style={{ marginTop: '20px' }}>
              <h4 style={{ margin: '0 0 15px 0', color: 'var(--color-accent)', fontSize: '16px' }}>
                📐 Solution #{selectedIdx + 1}
              </h4>

              <ArchitectureViewer
                allocation={result.pareto_front[selectedIdx].allocation}
                flows={lastContext?.flows || []}
                functionLoads={lastContext?.loads || {}}
                componentCapacities={lastContext?.caps || {}}
              />

              <div
                style={{
                  marginTop: '20px',
                  padding: '15px',
                  backgroundColor: 'rgba(0,0,0,0.2)',
                  borderRadius: '8px',
                }}
              >
                <h4 style={{ margin: '0 0 10px 0', fontSize: '14px' }}>Détails Liste</h4>
                <div style={{ maxHeight: '200px', overflowY: 'auto' }}>
                  <table style={styles.table}>
                    <thead>
                      <tr>
                        <th style={styles.th}>Fonction</th>
                        <th style={styles.th}>Alloc.</th>
                        <th style={styles.th}>Composant</th>
                      </tr>
                    </thead>
                    <tbody>
                      {result.pareto_front[selectedIdx].allocation.map(([f, c], i) => (
                        <tr key={i}>
                          <td style={styles.td}>
                            <span style={{ fontWeight: 'bold' }}>{f}</span>
                          </td>
                          {/* CORRECTION : Fusion des attributs style */}
                          <td style={{ ...styles.td, color: 'var(--text-muted)' }}>➜</td>
                          <td style={{ ...styles.td, color: 'var(--color-primary)' }}>{c}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
