import { useState, useRef, useEffect } from 'react'
import { collectionService } from '@/services/json-db/collection-service'
import { createQuery } from '@/services/json-db/query-service'
import { createTransaction, TransactionService } from '@/services/json-db/transaction-service'
import { Button } from '@/components/shared/Button'
import { InputBar } from '@/components/ai-chat/InputBar'
import type { OperationRequest } from '@/types/json-db.types'
import { invoke } from '@tauri-apps/api/core';

// Styles pour l'affichage
const OP_STYLES: Record<string, { color: string, label: string }> = {
  Insert: { color: '#4ade80', label: 'INSERT' },
  Update: { color: '#60a5fa', label: 'UPDATE' },
  Delete: { color: '#f87171', label: 'DELETE' }
};

const COLLECTION_NAME = 'smoke_test_transactions';

export function JsonDbTester() {
  const [activeTab, setActiveTab] = useState<'write' | 'search'>('write');
  const [logs, setLogs] = useState<string[]>([])
  const [items, setItems] = useState<any[]>([])
  const [searchQuery, setSearchQuery] = useState('')
  const [searchResults, setSearchResults] = useState<any[]>([])
  const [searchStats, setSearchStats] = useState<string>('')
  
  const [pendingOps, setPendingOps] = useState<OperationRequest[]>([])
  const txRef = useRef<TransactionService>(createTransaction())

  const addLog = (msg: string) => setLogs((prev) => [`[${new Date().toLocaleTimeString()}] ${msg}`, ...prev])

  useEffect(() => { initCollection(); }, []);

  const initCollection = async () => {
    try {
      await collectionService.createCollection(COLLECTION_NAME).catch(() => {});
      await refreshItems();
    } catch (e: any) {
      addLog(`⚠️ Erreur init: ${e}`);
    }
  }

  const refreshItems = async () => {
    try {
      const docs = await collectionService.listAll(COLLECTION_NAME);
      setItems(docs.reverse());
    } catch (e: any) {
      addLog(`⚠️ Erreur lecture: ${e}`);
    }
  }

  const handleSearch = async (text: string) => {
    if (!text.trim()) {
      setSearchResults([]); setSearchStats(''); return;
    }
    try {
      const start = performance.now();
      
      const query = createQuery(COLLECTION_NAME)
        .where('name', 'Contains', text)
        .orderBy('updatedAt', 'Desc')
        .limit(20)
        .build();

      // Appel avec 2 arguments : collection et query.
      // Le 3ème argument (options) est optionnel dans la définition du service ci-dessus.
      const results = await collectionService.queryDocuments(COLLECTION_NAME, query);
      
      const duration = (performance.now() - start).toFixed(2);
      setSearchResults(results);
      setSearchStats(`${results.length} résultat(s) en ${duration}ms`);
      addLog(`🔍 Recherche "${text}" : ${results.length} hits`);
    } catch (e: any) {
      addLog(`❌ Erreur recherche: ${e}`);
    }
  }

  const stageInsert = () => {
    const docName = `Doc ${items.length + pendingOps.length + 1}`;
    txRef.current.add(COLLECTION_NAME, { 
      name: docName, 
      status: 'draft', 
      updatedAt: new Date().toISOString()
    });
    updatePending();
  }

  const stageUpdate = (doc: any) => {
    txRef.current.update(COLLECTION_NAME, doc.id, { 
      ...doc, name: `${doc.name} (ed)`, status: 'published' 
    });
    updatePending();
  }

  const stageDelete = (id: string) => {
    txRef.current.delete(COLLECTION_NAME, id);
    updatePending();
  }

  const updatePending = () => setPendingOps([...txRef.current.getPendingOperations()]);

  const handleCommit = async () => {
    if (pendingOps.length === 0) return;
    try {
      await txRef.current.commit();
      addLog(`✅ Transaction Committed!`);
      txRef.current.rollback();
      updatePending();
      await refreshItems();
    } catch (e: any) {
      addLog(`❌ Transaction Failed: ${e}`);
    }
  }

  const testLoad = async () => {
    try {
      addLog("⏳ Chargement...");
      const start = performance.now();
      const model: any = await invoke('load_project_model', { space: 'un2', db: '_system' });
      const duration = (performance.now() - start).toFixed(2);
      addLog(`✅ Modèle chargé (${duration}ms). ${model.oa?.actors?.length || 0} acteurs.`);
    } catch (e: any) {
      addLog(`❌ Erreur chargement: ${e}`);
    }
  }

  const renderList = (data: any[], actions: boolean) => (
    <div style={{flex: 1, overflowY: 'auto', paddingTop: 10}}>
       {data.map(item => (
         <div key={item.id} style={{background: '#1f2937', marginBottom: 8, padding: 10, borderRadius: 6, display: 'flex', justifyContent: 'space-between'}}>
            <div>
              <div style={{color: '#fff'}}>{item.name}</div>
              <div style={{fontSize: '0.8em', color: '#9ca3af'}}>{item.id}</div>
            </div>
            {actions && (
              <div style={{display: 'flex', gap: 5}}>
                <button onClick={() => stageUpdate(item)} style={{color: '#60a5fa', background: 'none', border: '1px solid #60a5fa', borderRadius: 4, cursor: 'pointer'}}>Edit</button>
                <button onClick={() => stageDelete(item.id)} style={{color: '#f87171', background: 'none', border: '1px solid #f87171', borderRadius: 4, cursor: 'pointer'}}>Del</button>
              </div>
            )}
         </div>
       ))}
    </div>
  );

  return (
    <div style={{ padding: 20, background: '#111827', borderRadius: 8, border: '1px solid #374151', height: '600px', display: 'flex', flexDirection: 'column' }}>
      <div style={{display:'flex', justifyContent:'space-between', marginBottom: 15}}>
        <h3 style={{color:'white', margin:0}}>JSON-DB Tester</h3>
        <div style={{display:'flex', gap: 10}}>
          <button onClick={testLoad} style={{background:'#4f46e5', color:'white', border:'none', padding:'5px 10px', borderRadius:4, cursor:'pointer'}}>📂 Charger Modèle</button>
          <div style={{display:'flex', background: '#1f2937', borderRadius: 4}}>
             <button onClick={() => setActiveTab('write')} style={{background: activeTab==='write'?'#374151':'transparent', color: 'white', border:'none', padding:'5px 10px', cursor:'pointer'}}>Transactions</button>
             <button onClick={() => setActiveTab('search')} style={{background: activeTab==='search'?'#374151':'transparent', color: 'white', border:'none', padding:'5px 10px', cursor:'pointer'}}>Recherche</button>
          </div>
        </div>
      </div>

      <div style={{display: 'grid', gridTemplateColumns: '300px 1fr', gap: 20, flex: 1, overflow: 'hidden'}}>
        <div style={{display:'flex', flexDirection:'column', gap: 10}}>
           <div style={{background: '#1f2937', padding: 10, borderRadius: 8}}>
              <Button onClick={stageInsert} style={{width: '100%', marginBottom: 5}}>+ Nouveau</Button>
              <div style={{display: 'flex', gap: 5}}>
                <Button onClick={handleCommit} disabled={pendingOps.length===0} style={{flex:1, background: '#10b981'}}>Commit</Button>
                <Button onClick={() => { txRef.current.rollback(); updatePending(); }} disabled={pendingOps.length===0} variant="ghost" style={{flex:1, color: '#ef4444'}}>Clear</Button>
              </div>
           </div>
           <div style={{flex: 1, background: '#000', padding: 10, overflowY: 'auto', color: '#4ade80', fontSize: '0.8em', borderRadius: 8}}>
              {pendingOps.map((op, i) => (
                  <div key={i} style={{borderLeft: `3px solid ${OP_STYLES[op.type]?.color || '#fff'}`, paddingLeft: 5, marginBottom: 2}}>
                      {op.type}
                  </div>
              ))}
              {logs.map((l, i) => <div key={i}>{l}</div>)}
           </div>
        </div>

        <div style={{display:'flex', flexDirection:'column', height: '100%'}}>
           {activeTab === 'write' ? renderList(items, true) : (
             <>
               <InputBar value={searchQuery} onChange={setSearchQuery} onSend={handleSearch} placeholder="Rechercher..." />
               <div style={{marginTop: 5, color: '#10b981', fontSize: '0.8em', textAlign: 'right'}}>{searchStats}</div>
               {renderList(searchResults, false)}
             </>
           )}
        </div>
      </div>
    </div>
  );
}