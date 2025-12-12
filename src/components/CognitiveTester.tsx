import { useState } from 'react';
import { cognitiveService } from '@/services/cognitiveService';
import { AnalysisReport, CognitiveModel } from '@/types/cognitive.types';
import { useModelStore } from '@/store/model-store';

export default function CognitiveTester() {
  const [loading, setLoading] = useState(false);
  const [report, setReport] = useState<AnalysisReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Récupération du projet depuis le store global
  const currentProject = useModelStore((state) => state.project);

  // Modèle fictif de secours (Fallback)
  const dummyModel: CognitiveModel = {
    id: 'MOD-TEST-STATIC',
    elements: {
      'ERR-001': { name: '', kind: 'ErrorUnit', properties: {} },
      'OK-001': { name: 'ValidUnit', kind: 'Unit', properties: {} },
    },
    metadata: { author: 'Test User', version: '0.1' },
  };

  const handleRunAnalysis = async (useRealData: boolean) => {
    setLoading(true);
    setError(null);
    setReport(null);

    try {
      let dataToSend: CognitiveModel;

      if (useRealData && currentProject) {
        // --- ADAPTATEUR DE DONNÉES (Mapping) ---
        // On transforme le ProjectModel de l'app en CognitiveModel pour le WASM.
        // On utilise 'as any' pour accéder aux champs sans blocage TypeScript.
        const proj = currentProject as any;

        dataToSend = {
          // Si 'id' n'existe pas, on cherche 'name', 'handle' ou on met une valeur par défaut
          id: proj.id || proj.name || proj.handle || 'project-from-store',

          // Si 'elements' n'existe pas, on essaie de voir si c'est dans 'layers' ou 'nodes'
          // Sinon on envoie un objet vide pour ne pas faire crasher le WASM
          elements: proj.elements || proj.nodes || {},

          metadata: proj.metadata || {},
        };
      } else {
        dataToSend = dummyModel;
      }

      console.log('📤 Envoi au WASM :', dataToSend);

      // Appel du service
      const result = await cognitiveService.runConsistencyCheck(dataToSend);
      setReport(result);
    } catch (err) {
      console.error(err);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  // Styles
  const styles = {
    container: { padding: '20px', fontFamily: 'sans-serif' },
    card: {
      border: '1px solid #e0e0e0',
      borderRadius: '8px',
      padding: '20px',
      background: 'white',
      maxWidth: '800px',
      margin: '0 auto',
    },
    btnGroup: { display: 'flex', gap: '10px', marginTop: '15px' },
    btn: (primary: boolean) => ({
      padding: '10px 20px',
      cursor: 'pointer',
      background: primary ? 'var(--gradient-primary, #007bff)' : '#f0f0f0',
      color: primary ? 'white' : '#333',
      border: 'none',
      borderRadius: '6px',
      fontWeight: '500' as const,
    }),
    pre: {
      background: '#f8f9fa',
      padding: '15px',
      borderRadius: '6px',
      overflowX: 'auto' as const,
      fontSize: '12px',
      border: '1px solid #eee',
    },
  };

  // Helper pour l'affichage sécurisé (évite le crash si 'elements' n'existe pas)
  const getProjectInfo = () => {
    if (!currentProject) return 'Aucun projet';
    const p = currentProject as any;
    const id = p.id || p.name || 'ID Inconnu';
    const count = p.elements
      ? Object.keys(p.elements).length
      : p.nodes
      ? Object.keys(p.nodes).length
      : 0;
    return `${id} (${count} éléments)`;
  };

  return (
    <div style={styles.container}>
      <div style={styles.card}>
        <h2 style={{ marginTop: 0 }}>🧠 Moteur Cognitif (WASM)</h2>
        <p style={{ color: '#666' }}>
          Le moteur analyse la cohérence des données via le plugin{' '}
          <code>consistency_basic.wasm</code>.
        </p>

        <div
          style={{
            background: '#eef2ff',
            padding: '15px',
            borderRadius: '6px',
            border: '1px solid #c7d2fe',
          }}
        >
          <strong>Projet Actif : </strong>
          {currentProject ? (
            <span style={{ color: 'green', fontWeight: 'bold' }}>{getProjectInfo()}</span>
          ) : (
            <span style={{ color: 'orange' }}>Aucun projet chargé (Chargement en cours...)</span>
          )}
        </div>

        <div style={styles.btnGroup}>
          <button
            onClick={() => handleRunAnalysis(false)}
            disabled={loading}
            style={styles.btn(false)}
          >
            Tester avec Données Fictives
          </button>

          <button
            onClick={() => handleRunAnalysis(true)}
            disabled={loading || !currentProject}
            style={styles.btn(true)}
          >
            Analyser le Projet Réel
          </button>
        </div>

        {loading && (
          <div style={{ marginTop: '20px', color: '#007bff' }}>
            🔄 Analyse en cours par le noyau Rust/WASM...
          </div>
        )}

        {error && (
          <div style={{ marginTop: '20px', color: 'red', background: '#fff5f5', padding: '10px' }}>
            ❌ {error}
          </div>
        )}

        {report && (
          <div style={{ marginTop: '30px', borderTop: '2px solid #f0f0f0', paddingTop: '20px' }}>
            <div
              style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '15px' }}
            >
              <span style={{ fontSize: '1.2em', fontWeight: 'bold' }}>Rapport d'Analyse</span>
              <span
                style={{
                  padding: '4px 10px',
                  borderRadius: '20px',
                  fontSize: '0.85em',
                  fontWeight: 'bold',
                  background:
                    report.status === 'Success'
                      ? '#d1fae5'
                      : report.status === 'Warning'
                      ? '#fef3c7'
                      : '#fee2e2',
                  color:
                    report.status === 'Success'
                      ? '#065f46'
                      : report.status === 'Warning'
                      ? '#92400e'
                      : '#991b1b',
                }}
              >
                {report.status}
              </span>
            </div>

            {report.messages.length > 0 ? (
              <ul style={{ paddingLeft: '20px', color: '#444' }}>
                {report.messages.map((msg, idx) => (
                  <li
                    key={idx}
                    style={{
                      marginBottom: '5px',
                      color: msg.includes('ERREUR') ? '#dc2626' : 'inherit',
                    }}
                  >
                    {msg}
                  </li>
                ))}
              </ul>
            ) : (
              <p style={{ fontStyle: 'italic', color: '#888' }}>Aucune remarque.</p>
            )}

            <details>
              <summary
                style={{ cursor: 'pointer', color: '#888', fontSize: '0.9em', marginTop: '10px' }}
              >
                Détails techniques (JSON)
              </summary>
              <pre style={styles.pre}>{JSON.stringify(report, null, 2)}</pre>
            </details>
          </div>
        )}
      </div>
    </div>
  );
}
