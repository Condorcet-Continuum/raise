import { useState } from 'react';
import { geneticsService, GeneticsParams, OptimizationResult } from '@/services/geneticsService';
import { useModelStore } from '@/store/model-store';

export default function GeneticsDashboard() {
  const currentProject = useModelStore((state) => state.project);

  // Paramètres de simulation
  const [params, setParams] = useState<GeneticsParams>({
    population_size: 100,
    generations: 50,
    mutation_rate: 0.5,
  });

  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<OptimizationResult | null>(null);

  const handleRun = async () => {
    setLoading(true);
    setResult(null);
    try {
      const res = await geneticsService.runOptimization(params, currentProject || {});
      setResult(res);
    } catch (e) {
      alert("Erreur lors de l'optimisation : " + e);
    } finally {
      setLoading(false);
    }
  };

  // --- STYLES AVEC VARIABLES CSS ---
  const styles = {
    container: {
      padding: 'var(--spacing-4)',
      color: 'var(--text-main)',
      fontFamily: 'var(--font-family)',
      height: '100%',
      overflowY: 'auto' as const,
      backgroundColor: 'var(--bg-app)',
    },
    header: {
      marginBottom: 'var(--spacing-4)',
      borderBottom: '1px solid var(--border-color)',
      paddingBottom: 'var(--spacing-4)',
    },
    grid: {
      display: 'grid',
      gridTemplateColumns: '300px 1fr',
      gap: 'var(--spacing-4)',
    },
    panel: {
      backgroundColor: 'var(--bg-panel)',
      padding: 'var(--spacing-4)',
      borderRadius: 'var(--radius-lg)',
      border: '1px solid var(--border-color)',
      boxShadow: 'var(--shadow-sm)',
    },
    label: {
      display: 'block',
      marginBottom: 'var(--spacing-2)',
      fontSize: 'var(--font-size-sm)',
      color: 'var(--text-muted)',
      fontWeight: 'var(--font-weight-medium)',
    },
    inputGroup: { marginBottom: 'var(--spacing-4)' },
    range: {
      width: '100%',
      accentColor: 'var(--color-accent)', // Utilise la couleur d'accent (Violet)
      cursor: 'pointer',
    },
    valueDisplay: {
      float: 'right' as const,
      color: 'var(--color-accent)',
      fontWeight: 'bold',
      fontFamily: 'var(--font-family-mono)',
    },
    btn: {
      width: '100%',
      padding: '12px',
      // Gradient dynamique utilisant les variables
      background: 'linear-gradient(90deg, var(--color-accent), var(--color-primary))',
      color: '#ffffff',
      border: 'none',
      borderRadius: 'var(--radius-md)',
      fontWeight: 'var(--font-weight-bold)',
      cursor: loading ? 'not-allowed' : 'pointer',
      opacity: loading ? 0.7 : 1,
      transition: 'var(--transition-fast)',
      boxShadow: 'var(--shadow-md)',
    },
    statBox: {
      background: 'var(--bg-app)',
      padding: 'var(--spacing-4)',
      borderRadius: 'var(--radius-md)',
      flex: 1,
      border: '1px solid var(--border-color)',
    },
    chartContainer: {
      display: 'flex',
      alignItems: 'flex-end',
      gap: '4px',
      height: '200px',
      borderBottom: '1px solid var(--border-color)',
      paddingBottom: 'var(--spacing-2)',
      marginTop: 'var(--spacing-4)',
    },
  };

  // Fonction pour générer les barres du graphique dynamiquement
  const getBarHeight = (val: number) => ({
    height: `${Math.min(val, 100)}%`,
    width: '100%',
    backgroundColor: 'var(--color-accent)',
    opacity: 0.8,
    borderRadius: '2px 2px 0 0',
    transition: 'height 0.5s ease',
  });

  return (
    <div style={styles.container}>
      <header style={styles.header}>
        <h2 style={{ margin: 0, color: 'var(--color-accent)' }}>Optimisation Génétique</h2>
        <p style={{ color: 'var(--text-muted)', margin: '4px 0 0' }}>
          Exploration de l'espace de conception par sélection naturelle simulée.
        </p>
      </header>

      <div style={styles.grid}>
        {/* --- PANNEAU DE CONTRÔLE --- */}
        <div style={styles.panel}>
          <h3 style={{ marginTop: 0, fontSize: 'var(--font-size-lg)' }}>Paramètres</h3>

          <div style={styles.inputGroup}>
            <label style={styles.label}>
              Taille Population <span style={styles.valueDisplay}>{params.population_size}</span>
            </label>
            <input
              type="range"
              min="10"
              max="1000"
              step="10"
              style={styles.range}
              value={params.population_size}
              onChange={(e) => setParams({ ...params, population_size: +e.target.value })}
            />
          </div>

          <div style={styles.inputGroup}>
            <label style={styles.label}>
              Générations <span style={styles.valueDisplay}>{params.generations}</span>
            </label>
            <input
              type="range"
              min="10"
              max="500"
              step="10"
              style={styles.range}
              value={params.generations}
              onChange={(e) => setParams({ ...params, generations: +e.target.value })}
            />
          </div>

          <div style={styles.inputGroup}>
            <label style={styles.label}>
              Taux Mutation <span style={styles.valueDisplay}>{params.mutation_rate}</span>
            </label>
            <input
              type="range"
              min="0.1"
              max="1.0"
              step="0.1"
              style={styles.range}
              value={params.mutation_rate}
              onChange={(e) => setParams({ ...params, mutation_rate: +e.target.value })}
            />
          </div>

          <button style={styles.btn} onClick={handleRun} disabled={loading}>
            {loading ? '🧬 Évolution...' : "Lancer l'Optimisation"}
          </button>
        </div>

        {/* --- PANNEAU RÉSULTATS --- */}
        <div style={styles.panel}>
          <h3 style={{ marginTop: 0, fontSize: 'var(--font-size-lg)' }}>Convergence</h3>

          {!result && !loading && (
            <div
              style={{
                color: 'var(--text-muted)',
                fontStyle: 'italic',
                marginTop: 'var(--spacing-8)',
                textAlign: 'center',
                padding: 'var(--spacing-8)',
                border: '2px dashed var(--border-color)',
                borderRadius: 'var(--radius-md)',
              }}
            >
              Configurez les paramètres et lancez l'algorithme pour voir les résultats.
            </div>
          )}

          {loading && (
            <div style={{ textAlign: 'center', marginTop: 'var(--spacing-8)' }}>
              <div
                style={{
                  fontSize: '2.5rem',
                  marginBottom: 'var(--spacing-4)',
                  animation: 'pulse 1s infinite',
                }}
              >
                🧬
              </div>
              <p style={{ color: 'var(--text-muted)' }}>Calcul des générations en cours...</p>
            </div>
          )}

          {result && (
            <div>
              <div
                style={{
                  display: 'flex',
                  gap: 'var(--spacing-4)',
                  marginBottom: 'var(--spacing-4)',
                }}
              >
                <div style={styles.statBox}>
                  <div
                    style={{
                      fontSize: 'var(--font-size-xs)',
                      color: 'var(--text-muted)',
                      textTransform: 'uppercase',
                    }}
                  >
                    Meilleur Score
                  </div>
                  <div
                    style={{ fontSize: '1.5em', fontWeight: 'bold', color: 'var(--color-success)' }}
                  >
                    {result.best_score}%
                  </div>
                </div>
                <div style={styles.statBox}>
                  <div
                    style={{
                      fontSize: 'var(--font-size-xs)',
                      color: 'var(--text-muted)',
                      textTransform: 'uppercase',
                    }}
                  >
                    Durée
                  </div>
                  <div
                    style={{ fontSize: '1.5em', fontWeight: 'bold', color: 'var(--color-info)' }}
                  >
                    {result.duration_ms} ms
                  </div>
                </div>
                <div style={styles.statBox}>
                  <div
                    style={{
                      fontSize: 'var(--font-size-xs)',
                      color: 'var(--text-muted)',
                      textTransform: 'uppercase',
                    }}
                  >
                    Candidat ID
                  </div>
                  <div
                    style={{ fontSize: '1.2em', fontWeight: 'bold', color: 'var(--color-accent)' }}
                  >
                    {result.best_candidate_id}
                  </div>
                </div>
              </div>

              <h4
                style={{
                  margin: 'var(--spacing-4) 0 var(--spacing-2) 0',
                  color: 'var(--text-main)',
                }}
              >
                Historique de Convergence
              </h4>

              {/* Graphique */}
              <div style={styles.chartContainer}>
                {result.improvement_log.map((val, idx) => (
                  <div
                    key={idx}
                    style={getBarHeight(val)}
                    title={`Gen ${idx}: ${val.toFixed(1)}%`}
                  />
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
