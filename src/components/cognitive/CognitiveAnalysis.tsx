import { useState } from 'react';
import { useModelStore } from '@/store/model-store';
import { cognitiveService } from '@/services/cognitiveService';
import { Button } from '@/components/shared/Button';

// ✅ Imports corrigés qui correspondent maintenant au fichier de types
import type { AnalysisReport, CognitiveModel, ModelElement } from '@/types/cognitive.types';
import type { ProjectModel } from '@/types/model.types';

export default function CognitiveAnalysis() {
  const { project } = useModelStore();
  const [analyzing, setAnalyzing] = useState(false);
  const [report, setReport] = useState<AnalysisReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  // --- TRANSFORMATION ---
  const transformToCognitiveModel = (proj: ProjectModel): CognitiveModel => {
    // On initialise un objet vide typé
    const elements: Record<string, ModelElement> = {};

    // Fonction helper pour extraire les éléments de chaque couche
    const extractLayer = (layerName: string, items: any[]) => {
      if (!items) return;
      items.forEach((item) => {
        if (item && item.id) {
          elements[item.id] = {
            id: item.id,
            name: typeof item.name === 'string' ? item.name : 'Sans nom',
            kind: item.type || 'Unknown',
            properties: {
              layer: layerName,
              description: item.description || '',
            },
          };
        }
      });
    };

    // Extraction des couches
    if (proj.oa) extractLayer('OA', [...proj.oa.actors, ...proj.oa.activities]);
    if (proj.sa) extractLayer('SA', [...proj.sa.components, ...proj.sa.functions]);
    if (proj.la) extractLayer('LA', [...proj.la.components, ...proj.la.functions]);
    if (proj.pa) extractLayer('PA', [...proj.pa.components]);

    return {
      id: proj.id,
      elements,
      metadata: {
        projectName: proj.meta?.name || 'Projet Inconnu',
        version: proj.meta?.version || '1.0',
        timestamp: new Date().toISOString(),
      },
    };
  };

  // --- ACTION ---
  const runAnalysis = async () => {
    if (!project) {
      setError("Aucun projet chargé. Veuillez d'abord charger un modèle.");
      return;
    }

    setAnalyzing(true);
    setError(null);
    setReport(null);

    try {
      const payload = transformToCognitiveModel(project);

      // Appel au service (qui retourne Promise<AnalysisReport>)
      const result = await cognitiveService.runConsistencyCheck(payload);

      setReport(result);
    } catch (err: any) {
      console.error(err);
      setError('Erreur WASM : ' + (err.message || String(err)));
    } finally {
      setAnalyzing(false);
    }
  };

  // --- RENDU ---
  return (
    <div
      style={{
        padding: 'var(--spacing-6)',
        color: 'var(--text-main)',
        height: '100%',
        overflowY: 'auto',
      }}
    >
      <header
        style={{
          marginBottom: 'var(--spacing-6)',
          borderBottom: '1px solid var(--border-color)',
          paddingBottom: 'var(--spacing-4)',
        }}
      >
        <h2 style={{ margin: 0, color: 'var(--color-info)' }}>Moteur Cognitif (Rust/WASM)</h2>
        <p style={{ color: 'var(--text-muted)', marginTop: '5px' }}>
          Exécution de plugins WebAssembly pour l'analyse structurelle et sémantique.
        </p>
      </header>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 350px', gap: 'var(--spacing-6)' }}>
        {/* COLONNE GAUCHE : Rapport */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-4)' }}>
          {error && (
            <div
              style={{
                padding: '15px',
                backgroundColor: 'rgba(239, 68, 68, 0.1)',
                border: '1px solid var(--color-error)',
                borderRadius: 'var(--radius-md)',
                color: 'var(--color-error)',
              }}
            >
              🛑 {error}
            </div>
          )}

          {!report && !analyzing && !error && (
            <div
              style={{
                backgroundColor: 'var(--bg-panel)',
                padding: '40px',
                borderRadius: 'var(--radius-lg)',
                border: '1px dashed var(--border-color)',
                textAlign: 'center',
                color: 'var(--text-muted)',
              }}
            >
              <div style={{ fontSize: '3rem', marginBottom: '10px' }}>📦 ➡️ 🦀 ➡️ 🧠</div>
              <p>Le modèle sera sérialisé et envoyé au moteur Rust (WASM).</p>
            </div>
          )}

          {analyzing && (
            <div style={{ textAlign: 'center', padding: '50px' }}>
              <p>Analyse en cours...</p>
            </div>
          )}

          {report && (
            <div
              style={{
                backgroundColor: 'var(--bg-panel)',
                padding: 'var(--spacing-6)',
                borderRadius: 'var(--radius-lg)',
                border: '1px solid var(--border-color)',
              }}
            >
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  marginBottom: '20px',
                }}
              >
                <h3 style={{ margin: 0 }}>Rapport du Plugin</h3>
                <span
                  style={{
                    backgroundColor: 'var(--bg-app)',
                    padding: '4px 8px',
                    borderRadius: '4px',
                    fontSize: '0.8rem',
                    fontFamily: 'monospace',
                  }}
                >
                  ID: {report.block_id}
                </span>
              </div>

              {report.messages.length === 0 ? (
                <div style={{ color: 'var(--color-success)', fontStyle: 'italic' }}>
                  Aucune anomalie détectée.
                </div>
              ) : (
                <ul
                  style={{
                    paddingLeft: '20px',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '8px',
                  }}
                >
                  {report.messages.map((msg, idx) => (
                    <li key={idx}>{msg}</li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </div>

        {/* COLONNE DROITE : Score & Actions */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-4)' }}>
          <div
            style={{
              backgroundColor: 'var(--bg-panel)',
              padding: 'var(--spacing-6)',
              borderRadius: 'var(--radius-lg)',
              border: '1px solid var(--border-color)',
              textAlign: 'center',
              boxShadow: 'var(--shadow-md)',
            }}
          >
            <div
              style={{
                fontSize: '0.8rem',
                fontWeight: 'bold',
                color: 'var(--text-muted)',
                marginBottom: '10px',
                textTransform: 'uppercase',
              }}
            >
              Statut Global
            </div>
            <div
              style={{
                fontSize: '2.5rem',
                fontWeight: 'bold',
                color: report
                  ? report.status === 'Success'
                    ? 'var(--color-success)'
                    : report.status === 'Warning'
                    ? 'var(--color-warning)'
                    : 'var(--color-error)'
                  : 'var(--text-muted)',
              }}
            >
              {report ? report.status : '--'}
            </div>
          </div>

          <div
            style={{
              backgroundColor: 'var(--bg-panel)',
              padding: 'var(--spacing-4)',
              borderRadius: 'var(--radius-lg)',
              border: '1px solid var(--border-color)',
            }}
          >
            <Button
              variant="primary"
              onClick={runAnalysis}
              disabled={analyzing || !project}
              style={{ width: '100%' }}
            >
              {analyzing ? 'Traitement...' : "▶ Exécuter l'analyse"}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
