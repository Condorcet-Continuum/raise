import { useState, useEffect, useCallback } from 'react';
import { collectionService } from '@/services/json-db/collection-service';
import { createQuery } from '@/services/json-db/query-service';
import { modelService } from '@/services/model-service';
import { useModelStore } from '@/store/model-store';
import { InputBar } from '@/components/ai-chat/InputBar';

// Type pour les documents JSON génériques
type JsonDoc = Record<string, unknown>;
type TabType = 'write' | 'search' | 'admin';

export function JsonDbTester() {
  const [logs, setLogs] = useState<string[]>([]);
  const [activeTab, setActiveTab] = useState<TabType>('admin');

  const [targetCollection, setTargetCollection] = useState('actors');
  const [items, setItems] = useState<JsonDoc[]>([]);

  const [jsonInput, setJsonInput] = useState<string>(
    JSON.stringify(
      {
        '@context': { oa: 'https://raise.io/ontology/arcadia/oa#' },
        '@type': 'oa:OperationalActor',
        name: 'Opérateur de Drone',
        description: 'Personne chargée du pilotage.',
      },
      null,
      2,
    ),
  );

  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<JsonDoc[]>([]);
  const [searchStats, setSearchStats] = useState<string>('');

  const setProject = useModelStore((s) => s.setProject);

  const addLog = (msg: string) =>
    setLogs((prev) => [`[${new Date().toLocaleTimeString()}] ${msg}`, ...prev]);

  const fetchCollectionData = useCallback(async (colName: string): Promise<JsonDoc[]> => {
    try {
      const docs = await collectionService.listAll(colName);
      return Array.isArray(docs) ? (docs as JsonDoc[]).reverse() : [];
    } catch {
      return [];
    }
  }, []);

  useEffect(() => {
    let isMounted = true;
    const load = async () => {
      const data = await fetchCollectionData(targetCollection);
      if (isMounted) setItems(data);
    };
    load();
    return () => {
      isMounted = false;
    };
  }, [targetCollection, fetchCollectionData]);

  const handleRefreshManual = async () => {
    const data = await fetchCollectionData(targetCollection);
    setItems(data);
  };

  const handleInitDb = async () => {
    try {
      await collectionService.createDb();
      addLog('✅ Base de données initialisée.');
    } catch (e: unknown) {
      addLog(`❌ Erreur Init DB: ${String(e)}`);
    }
  };

  const handleDropDb = async () => {
    if (!confirm('Êtes-vous sûr ?')) return;
    try {
      await collectionService.dropDb();
      addLog('🗑️ Base de données supprimée.');
      setItems([]);
    } catch (e: unknown) {
      addLog(`❌ Erreur Drop DB: ${String(e)}`);
    }
  };

  const handleCreateIndex = async () => {
    try {
      await collectionService.createIndex(targetCollection, 'name', 'hash');
      addLog(`✅ Index Hash créé sur ${targetCollection}.name`);
    } catch (e: unknown) {
      addLog(`❌ Erreur Index: ${String(e)}`);
    }
  };

  const handleCreateCollection = async () => {
    try {
      await collectionService.createCollection(targetCollection);
      addLog(`✅ Collection '${targetCollection}' prête.`);
      await handleRefreshManual();
    } catch (e: unknown) {
      addLog(`❌ Erreur création collection: ${String(e)}`);
    }
  };

  const handleInsertJson = async () => {
    if (!targetCollection.trim()) return;
    try {
      const doc = JSON.parse(jsonInput);
      const saved = await collectionService.insertDocument(targetCollection, doc);
      const id = (saved as { id?: string }).id || '?';
      addLog(`✅ Document inséré (ID: ${id})`);
      await handleRefreshManual();
    } catch (e: unknown) {
      addLog(`❌ Erreur insertion: ${String(e)}`);
    }
  };

  const handleSearch = async (text: string) => {
    if (!text.trim()) {
      setSearchResults([]);
      return;
    }
    try {
      const start = performance.now();
      const query = createQuery(targetCollection).where('name', 'Contains', text).limit(20).build();
      const results = await collectionService.queryDocuments(targetCollection, query);
      const duration = (performance.now() - start).toFixed(2);

      setSearchResults(results as JsonDoc[]);
      setSearchStats(`${Array.isArray(results) ? results.length : 0} résultat(s) en ${duration}ms`);
      addLog(`🔍 Recherche "${text}" terminée.`);
    } catch (e: unknown) {
      addLog(`❌ Erreur recherche: ${String(e)}`);
    }
  };

  const handleLoadModel = async () => {
    try {
      addLog('⏳ Chargement du modèle...');
      const model = await modelService.loadProjectModel('un2', '_system');
      setProject(model);
      addLog(`✅ Modèle chargé !`);
    } catch (e: unknown) {
      addLog(`❌ Erreur modèle: ${String(e)}`);
    }
  };

  const tabs: TabType[] = ['admin', 'write', 'search'];

  return (
    <div
      style={{
        padding: 20,
        background: '#111827',
        borderRadius: 8,
        border: '1px solid #374151',
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 15 }}>
        <h3 style={{ color: 'white', margin: 0 }}>JSON-DB Explorer</h3>
        <div style={{ display: 'flex', gap: 5 }}>
          {tabs.map((tab) => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              style={{
                padding: '6px 12px',
                borderRadius: 4,
                border: 'none',
                cursor: 'pointer',
                background: activeTab === tab ? '#4f46e5' : '#374151',
                color: 'white',
                textTransform: 'capitalize',
              }}
            >
              {tab}
            </button>
          ))}
        </div>
      </div>

      <div style={{ display: 'flex', gap: 10, marginBottom: 15 }}>
        <input
          value={targetCollection}
          onChange={(e) => setTargetCollection(e.target.value)}
          placeholder="Collection"
          style={{
            flex: 1,
            padding: 8,
            borderRadius: 4,
            border: '1px solid #374151',
            background: '#1f2937',
            color: 'white',
          }}
        />
        <button
          onClick={handleCreateCollection}
          style={{
            background: '#10b981',
            color: 'white',
            border: 'none',
            borderRadius: 4,
            padding: '0 15px',
            cursor: 'pointer',
          }}
        >
          Ouvrir
        </button>
      </div>

      <div style={{ flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        {activeTab === 'admin' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 15 }}>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
              <button
                onClick={handleInitDb}
                style={{
                  padding: 15,
                  background: '#059669',
                  color: 'white',
                  border: 'none',
                  borderRadius: 6,
                  cursor: 'pointer',
                }}
              >
                🏗️ Init DB
              </button>
              <button
                onClick={handleDropDb}
                style={{
                  padding: 15,
                  background: '#dc2626',
                  color: 'white',
                  border: 'none',
                  borderRadius: 6,
                  cursor: 'pointer',
                }}
              >
                💥 Drop DB
              </button>
              <button
                onClick={handleCreateIndex}
                style={{
                  padding: 15,
                  background: '#d97706',
                  color: 'white',
                  border: 'none',
                  borderRadius: 6,
                  cursor: 'pointer',
                }}
              >
                ⚡ Index
              </button>
              <button
                onClick={handleLoadModel}
                style={{
                  padding: 15,
                  background: '#2563eb',
                  color: 'white',
                  border: 'none',
                  borderRadius: 6,
                  cursor: 'pointer',
                }}
              >
                🧠 Load Model
              </button>
            </div>
            <div
              style={{
                flex: 1,
                background: '#000',
                padding: 10,
                overflowY: 'auto',
                color: '#4ade80',
                fontSize: '0.8em',
                fontFamily: 'monospace',
                borderRadius: 6,
                border: '1px solid #374151',
              }}
            >
              {logs.map((l, i) => (
                <div key={i}>{l}</div>
              ))}
            </div>
          </div>
        )}

        {activeTab === 'write' && (
          <div style={{ display: 'flex', flexDirection: 'column', height: '100%', gap: 10 }}>
            <textarea
              value={jsonInput}
              onChange={(e) => setJsonInput(e.target.value)}
              style={{
                flex: 1,
                background: '#000',
                color: '#a5f3fc',
                fontFamily: 'monospace',
                padding: 10,
                borderRadius: 6,
                border: '1px solid #374151',
                resize: 'none',
              }}
            />
            <button
              onClick={handleInsertJson}
              style={{
                background: '#3b82f6',
                color: 'white',
                border: 'none',
                padding: 12,
                borderRadius: 6,
                cursor: 'pointer',
                fontWeight: 'bold',
              }}
            >
              💾 Insérer
            </button>
          </div>
        )}

        {activeTab === 'search' && (
          <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
            <InputBar
              value={searchQuery}
              onChange={setSearchQuery}
              onSend={handleSearch}
              placeholder={`Rechercher...`}
            />
            <div
              style={{
                marginTop: 5,
                color: '#10b981',
                fontSize: '0.8em',
                textAlign: 'right',
                minHeight: '20px',
              }}
            >
              {searchStats}
            </div>
            <div style={{ flex: 1, overflowY: 'auto', marginTop: 10 }}>
              {/* CORRECTION ICI : Utilisation de l'index comme clé de repli au lieu de Math.random() */}
              {(searchResults.length > 0 ? searchResults : items).map((item, index) => (
                <div
                  key={String(item.id || index)}
                  style={{
                    background: '#1f2937',
                    marginBottom: 8,
                    padding: 10,
                    borderRadius: 6,
                    borderLeft: '3px solid #6366f1',
                  }}
                >
                  <div style={{ color: '#fff', fontWeight: 'bold' }}>
                    {String(item.name || item.title || 'Sans nom')}
                  </div>
                  <div style={{ fontSize: '0.8em', color: '#9ca3af', marginTop: 4 }}>
                    {String(item.description || '')}
                  </div>
                  <div
                    style={{
                      fontSize: '0.7em',
                      color: '#6b7280',
                      marginTop: 6,
                      fontFamily: 'monospace',
                    }}
                  >
                    ID: {String(item.id)}
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
