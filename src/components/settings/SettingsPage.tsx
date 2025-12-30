import { useState } from 'react';
import { useSettingsStore, AiBackend } from '@/store/settings-store';
import { useModelStore } from '@/store/model-store';
import { modelService } from '@/services/model-service';
import { Button } from '@/components/shared/Button';
import { parseError } from '@/utils/parsers';

// ✅ IMPORT DU BOUTON D'EXPORT
import AiExportButton from '@/components/ai-chat/AiExportButton';

export default function SettingsPage() {
  const settings = useSettingsStore();
  const setProject = useModelStore((state) => state.setProject);

  const [loading, setLoading] = useState(false);
  const [msg, setMsg] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  // Fonction pour tester la connexion à la DB et recharger le modèle
  const handleReloadModel = async () => {
    setLoading(true);
    setMsg(null);
    try {
      // Appel au service connecté au backend Rust
      const project = await modelService.loadProjectModel(
        settings.jsonDbSpace,
        settings.jsonDbDatabase,
      );
      setProject(project);
      setMsg({
        type: 'success',
        text: `Projet "${project.meta?.name || project.id}" chargé avec succès !`,
      });
    } catch (err) {
      console.error(err);
      setMsg({
        type: 'error',
        text: `Échec du chargement : ${parseError(err)} (Le backend Rust est-il lancé ?)`,
      });
    } finally {
      setLoading(false);
    }
  };

  const sectionStyle = {
    backgroundColor: 'var(--bg-panel)',
    padding: 'var(--spacing-6)',
    borderRadius: 'var(--radius-lg)',
    border: '1px solid var(--border-color)',
    marginBottom: 'var(--spacing-6)',
    maxWidth: '800px',
  };

  const labelStyle = {
    display: 'block',
    marginBottom: 'var(--spacing-2)',
    fontWeight: 'var(--font-weight-medium)',
    color: 'var(--text-main)',
  };

  const inputStyle = {
    width: '100%',
    padding: '8px 12px',
    borderRadius: 'var(--radius-md)',
    border: '1px solid var(--border-color)',
    backgroundColor: 'var(--bg-app)',
    color: 'var(--text-main)',
    marginBottom: 'var(--spacing-4)',
    fontSize: 'var(--font-size-sm)',
  };

  return (
    <div style={{ padding: 'var(--spacing-6)' }}>
      <h2 style={{ marginBottom: 'var(--spacing-6)', color: 'var(--text-main)' }}>
        Paramètres du Système
      </h2>

      {/* SECTION IA : CONFIGURATION */}
      <div style={sectionStyle}>
        <h3 style={{ marginTop: 0, color: 'var(--color-primary)' }}>
          🤖 Intelligence Artificielle
        </h3>
        <p
          style={{
            color: 'var(--text-muted)',
            fontSize: '0.9em',
            marginBottom: 'var(--spacing-4)',
          }}
        >
          Configurez le moteur utilisé par l'assistant de chat et les agents.
        </p>

        <label style={labelStyle}>Backend IA</label>
        <select
          value={settings.aiBackend}
          onChange={(e) => settings.update({ aiBackend: e.target.value as AiBackend })}
          style={inputStyle}
        >
          <option value="mock">Simulation (Mock) - Pour dév UI sans backend</option>
          <option value="tauri-local">Local LLM (Ollama/Rust) - Via Tauri IPC</option>
          <option value="remote-api">Remote API (OpenAI/Mistral) - Via HTTPS</option>
        </select>

        <div style={{ fontSize: '0.85em', color: 'var(--color-info)' }}>
          Mode actuel : <strong>{settings.aiBackend}</strong>
        </div>
      </div>

      {/* ✅ SECTION IA : ENTRAÎNEMENT (NOUVEAU) */}
      <div style={sectionStyle}>
        <h3 style={{ marginTop: 0, color: '#8b5cf6' }}>
          {' '}
          {/* Une couleur violette pour distinguer l'IA générative */}
          🧠 Entraînement (Fine-Tuning)
        </h3>
        <p
          style={{
            color: 'var(--text-muted)',
            fontSize: '0.9em',
            marginBottom: 'var(--spacing-4)',
          }}
        >
          Générez un dataset d'entraînement à partir de vos données actuelles pour spécialiser le
          modèle.
        </p>

        {/* Le composant Bouton gère lui-même son état de chargement */}
        <AiExportButton />
      </div>

      {/* SECTION BASE DE DONNÉES */}
      <div style={sectionStyle}>
        <h3 style={{ marginTop: 0, color: 'var(--color-accent)' }}>🗄️ Base de Données (JSON-DB)</h3>
        <p
          style={{
            color: 'var(--text-muted)',
            fontSize: '0.9em',
            marginBottom: 'var(--spacing-4)',
          }}
        >
          Ciblez l'espace de données où sont stockés vos modèles Arcadia.
        </p>

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 'var(--spacing-4)' }}>
          <div>
            <label style={labelStyle}>Espace (Space)</label>
            <input
              type="text"
              value={settings.jsonDbSpace}
              onChange={(e) => settings.update({ jsonDbSpace: e.target.value })}
              style={inputStyle}
            />
          </div>
          <div>
            <label style={labelStyle}>Base (Database)</label>
            <input
              type="text"
              value={settings.jsonDbDatabase}
              onChange={(e) => settings.update({ jsonDbDatabase: e.target.value })}
              style={inputStyle}
            />
          </div>
        </div>

        <div style={{ marginTop: 'var(--spacing-2)' }}>
          <Button onClick={handleReloadModel} disabled={loading} variant="primary">
            {loading ? 'Connexion...' : 'Tester & Recharger le Modèle'}
          </Button>
        </div>

        {msg && (
          <div
            style={{
              marginTop: 'var(--spacing-4)',
              padding: '10px',
              borderRadius: 'var(--radius-md)',
              backgroundColor:
                msg.type === 'success' ? 'rgba(16, 185, 129, 0.1)' : 'rgba(239, 68, 68, 0.1)',
              color: msg.type === 'success' ? 'var(--color-success)' : 'var(--color-error)',
              border: `1px solid ${
                msg.type === 'success' ? 'var(--color-success)' : 'var(--color-error)'
              }`,
            }}
          >
            {msg.text}
          </div>
        )}
      </div>
    </div>
  );
}
