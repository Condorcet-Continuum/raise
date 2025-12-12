import { useState } from 'react';
import { codegenService } from '@/services/codegenService';
import { useModelStore } from '@/store/model-store';

export default function CodeGenerator() {
  const [language, setLanguage] = useState('rust');
  const [code, setCode] = useState('// Le code généré apparaîtra ici...');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const currentProject = useModelStore((state) => state.project);

  const handleGenerate = async () => {
    if (!currentProject) {
      setError("Aucun modèle chargé. Veuillez d'abord charger un projet.");
      return;
    }

    setLoading(true);
    setError(null);
    setCopied(false);

    try {
      const result = await codegenService.generateCode(language, currentProject);
      setCode(result);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleCopy = () => {
    navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // --- STYLES AVEC VARIABLES CSS ---
  const styles = {
    container: {
      height: '100%',
      display: 'flex',
      flexDirection: 'column' as const,
      gap: 'var(--spacing-4)',
      padding: 'var(--spacing-4)',
      fontFamily: 'var(--font-family)',
      backgroundColor: 'var(--bg-panel)',
      color: 'var(--text-main)',
      borderRadius: 'var(--radius-lg)',
      border: '1px solid var(--border-color)',
    },
    header: {
      borderBottom: '1px solid var(--border-color)',
      paddingBottom: 'var(--spacing-4)',
      marginBottom: 'var(--spacing-2)',
    },
    toolbar: {
      display: 'flex',
      justifyContent: 'space-between',
      alignItems: 'center',
      padding: 'var(--spacing-2) var(--spacing-4)',
      backgroundColor: 'var(--color-gray-50)', // Légère différence avec le fond
      borderRadius: 'var(--radius-md) var(--radius-md) 0 0',
      border: '1px solid var(--border-color)',
      borderBottom: 'none',
    },
    controls: { display: 'flex', gap: 'var(--spacing-2)', alignItems: 'center' },
    select: {
      backgroundColor: 'var(--bg-panel)',
      color: 'var(--text-main)',
      border: '1px solid var(--border-color)',
      padding: '6px 12px',
      borderRadius: 'var(--radius-sm)',
      cursor: 'pointer',
      outline: 'none',
      fontWeight: 'var(--font-weight-medium)',
      fontSize: 'var(--font-size-sm)',
      fontFamily: 'var(--font-family)',
    },
    btnGen: {
      backgroundColor: 'var(--color-primary)',
      color: '#ffffff',
      border: 'none',
      padding: '6px 16px',
      borderRadius: 'var(--radius-sm)',
      cursor: 'pointer',
      fontWeight: 'var(--font-weight-semibold)',
      display: 'flex',
      alignItems: 'center',
      gap: '6px',
      fontSize: 'var(--font-size-sm)',
      transition: 'var(--transition-fast)',
    },
    btnCopy: {
      backgroundColor: copied ? 'var(--color-success)' : 'var(--bg-panel)',
      color: copied ? '#ffffff' : 'var(--text-main)',
      border: '1px solid var(--border-color)',
      padding: '6px 12px',
      borderRadius: 'var(--radius-sm)',
      cursor: 'pointer',
      transition: 'var(--transition-fast)',
      fontSize: 'var(--font-size-sm)',
      fontWeight: 'var(--font-weight-medium)',
    },
    editor: {
      flex: 1,
      // En mode light: fond gris clair (gray-50). En dark mode : fond gris foncé (gray-900)
      // On utilise var(--bg-app) qui gère cette inversion naturellement
      backgroundColor: 'var(--bg-app)',
      color: 'var(--text-main)',
      padding: 'var(--spacing-4)',
      borderRadius: '0 0 var(--radius-md) var(--radius-md)',
      overflow: 'auto',
      fontFamily: 'var(--font-family-mono)',
      fontSize: 'var(--font-size-sm)',
      lineHeight: '1.6',
      border: '1px solid var(--border-color)',
      whiteSpace: 'pre' as const,
    },
    status: {
      fontSize: 'var(--font-size-xs)',
      color: 'var(--text-muted)',
      marginTop: 'var(--spacing-2)',
    },
  };

  return (
    <div style={styles.container}>
      <header style={styles.header}>
        <h2 style={{ margin: 0, color: 'var(--color-primary)', fontSize: 'var(--font-size-xl)' }}>
          Usine Logicielle
        </h2>
        <p
          style={{ margin: '5px 0 0', color: 'var(--text-muted)', fontSize: 'var(--font-size-sm)' }}
        >
          Générez du code infrastructurel à partir de votre modèle d'architecture.
        </p>
      </header>

      {/* --- BARRE D'OUTILS --- */}
      <div style={{ display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0 }}>
        <div style={styles.toolbar}>
          <div style={styles.controls}>
            <span
              style={{
                color: 'var(--text-muted)',
                fontSize: 'var(--font-size-sm)',
                fontWeight: 600,
              }}
            >
              Cible :
            </span>
            <select
              value={language}
              onChange={(e) => setLanguage(e.target.value)}
              style={styles.select}
            >
              <option value="rust">🦀 Rust (System)</option>
              <option value="python">🐍 Python (Scripting)</option>
              <option value="cpp">⚙️ C++ (Embedded)</option>
            </select>

            <button
              onClick={handleGenerate}
              disabled={loading}
              style={{
                ...styles.btnGen,
                opacity: loading ? 0.7 : 1,
                cursor: loading ? 'not-allowed' : 'pointer',
              }}
            >
              {loading ? 'Construction...' : '⚡ Générer'}
            </button>
          </div>

          <button onClick={handleCopy} disabled={!code} style={styles.btnCopy}>
            {copied ? 'Copié !' : '📋 Copier'}
          </button>
        </div>

        {/* --- ZONE ÉDITEUR --- */}
        <div style={styles.editor}>
          {error ? (
            <div style={{ color: 'var(--color-error)' }}>❌ Erreur : {error}</div>
          ) : (
            <code>{code}</code>
          )}
        </div>
      </div>

      <div style={styles.status}>
        Moteur de templates : <strong>Tera</strong> • Modèle actif :{' '}
        <strong>{currentProject ? (currentProject as any).id || 'Inconnu' : 'Aucun'}</strong>
      </div>
    </div>
  );
}
