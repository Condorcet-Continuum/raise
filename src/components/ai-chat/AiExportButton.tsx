import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useSettingsStore } from '@/store/settings-store'; // ✅ Pour récupérer la config active

export default function AiExportButton() {
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  // ✅ On récupère l'espace et la DB configurés par l'utilisateur
  const { jsonDbSpace, jsonDbDatabase } = useSettingsStore();

  const handleExport = async () => {
    setLoading(true);
    setStatus(null);

    try {
      // ✅ On envoie les 3 arguments requis par Rust
      const response = await invoke<string>('ai_export_dataset', {
        outputPath: 'dataset.jsonl',
        space: jsonDbSpace || 'un2', // Valeur par défaut si vide
        dbName: jsonDbDatabase || 'default', // "dbName" sera converti en "db_name" par Tauri
      });

      setStatus(`✅ ${response}`);
    } catch (error) {
      console.error(error);
      setStatus(`❌ Erreur: ${typeof error === 'string' ? error : JSON.stringify(error)}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col items-start gap-2 p-4 border rounded-lg bg-gray-50 shadow-sm">
      <h3 className="font-bold text-gray-700" style={{ margin: 0 }}>
        Entraînement IA
      </h3>
      <p className="text-sm text-gray-500 mb-2" style={{ margin: '5px 0 10px 0' }}>
        Générez le dataset depuis l'espace <strong>{jsonDbSpace}</strong> /{' '}
        <strong>{jsonDbDatabase}</strong>.
      </p>

      <button
        onClick={handleExport}
        disabled={loading}
        style={{
          padding: '8px 16px',
          backgroundColor: loading ? '#9ca3af' : '#7c3aed', // Violet pour matcher le thème "Entraînement"
          color: 'white',
          border: 'none',
          borderRadius: '4px',
          cursor: loading ? 'not-allowed' : 'pointer',
          fontWeight: 500,
        }}
      >
        {loading ? 'Extraction en cours...' : '🚀 Exporter le Dataset'}
      </button>

      {status && (
        <div
          style={{
            marginTop: '10px',
            fontSize: '0.9em',
            color: status.startsWith('✅') ? '#16a34a' : '#dc2626',
          }}
        >
          {status}
        </div>
      )}
    </div>
  );
}
