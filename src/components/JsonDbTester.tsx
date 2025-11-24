import { useState } from 'react'
import { collectionService } from '@/services/json-db/collection-service'
import { Button } from '@/components/shared/Button'

export function JsonDbTester() {
  const [logs, setLogs] = useState<string[]>([])
  const [items, setItems] = useState<any[]>([])

  const addLog = (msg: string) => setLogs((prev) => [`[${new Date().toLocaleTimeString()}] ${msg}`, ...prev])

  const runTest = async () => {
    try {
      const collName = 'smoke_test_items'
      
      addLog(`1. Création de la collection '${collName}'...`)
      await collectionService.createCollection(collName)
      addLog('✅ Collection créée (ou déjà existante)')

      const newDoc = { 
        title: 'Hello from Tauri', 
        timestamp: Date.now(),
        type: 'smoke-test' 
      }
      
      addLog('2. Insertion d\'un document...')
      await collectionService.insertRaw(collName, newDoc)
      addLog('✅ Document inséré')

      addLog('3. Lecture de la collection...')
      const docs = await collectionService.listAll(collName)
      setItems(docs)
      addLog(`✅ ${docs.length} document(s) récupéré(s)`)

    } catch (error: any) {
      addLog(`❌ ERREUR: ${error}`)
      console.error(error)
    }
  }

  return (
    <div style={{ padding: 20, background: '#111827', borderRadius: 8, border: '1px solid #374151', marginTop: 20 }}>
      <h3 style={{ color: '#fff', marginTop: 0 }}>🔥 JSON-DB Smoke Test</h3>
      
      <div style={{ marginBottom: 16 }}>
        {/* Correction ici : on remplace les flèches -> par des entités ou des chaînes sécurisées */}
        <Button onClick={runTest}>Lancer le Test (Create &rarr; Insert &rarr; List)</Button>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
        {/* Logs */}
        <div style={{ background: '#000', padding: 10, borderRadius: 4, height: 200, overflowY: 'auto', fontSize: '0.85em', fontFamily: 'monospace', color: '#4ade80' }}>
          {logs.length === 0 && <span style={{opacity:0.5}}>En attente...</span>}
          {logs.map((l, i) => <div key={i}>{l}</div>)}
        </div>

        {/* Data Visualizer */}
        <div style={{ background: '#1f2937', padding: 10, borderRadius: 4, height: 200, overflowY: 'auto', fontSize: '0.85em', color: '#e5e7eb' }}>
          <pre>{JSON.stringify(items, null, 2)}</pre>
        </div>
      </div>
    </div>
  )
}