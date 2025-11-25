import { useState, useRef, useEffect } from 'react'
import { collectionService } from '@/services/json-db/collection-service'
import { createTransaction, TransactionService } from '@/services/json-db/transaction-service'
import { Button } from '@/components/shared/Button'
import type { OperationRequest } from '@/types/json-db.types'

// Styles pour les badges d'opération
const OP_STYLES = {
  insert: { color: '#4ade80', label: 'INSERT' }, // Vert
  update: { color: '#60a5fa', label: 'UPDATE' }, // Bleu
  delete: { color: '#f87171', label: 'DELETE' }  // Rouge
};

export function JsonDbTester() {
  const [logs, setLogs] = useState<string[]>([])
  const [items, setItems] = useState<any[]>([])
  
  // État local pour l'affichage des opérations en attente
  const [pendingOps, setPendingOps] = useState<OperationRequest[]>([])
  
  // Référence stable vers le service de transaction
  const txRef = useRef<TransactionService>(createTransaction())

  const addLog = (msg: string) => setLogs((prev) => [`[${new Date().toLocaleTimeString()}] ${msg}`, ...prev])

  // Chargement initial
  useEffect(() => {
    refreshItems();
  }, []);

  // Rafraîchir la liste des items réels
  const refreshItems = async () => {
    try {
      // On s'assure que la collection existe pour éviter une erreur au premier lancement
      await collectionService.createCollection('smoke_test_transactions').catch(() => {});
      const docs = await collectionService.listAll('smoke_test_transactions');
      // Tri par date décroissante pour voir les derniers ajouts en haut
      setItems(docs.reverse());
    } catch (e: any) {
      addLog(`⚠️ Erreur lecture: ${e}`);
    }
  }

  // --- Actions Transactionnelles (STAGING) ---

  const stageInsert = () => {
    const collName = 'smoke_test_transactions';
    // On laisse le backend générer l'ID ou on en met un temporaire
    const docName = `Document ${items.length + pendingOps.length + 1}`;
    
    txRef.current.add(collName, { 
      name: docName, 
      status: 'draft',
      updatedAt: new Date().toISOString()
    });
    
    addLog(`📝 Staged: INSERT "${docName}"`);
    setPendingOps(txRef.current.getPendingOperations());
  }

  const stageUpdate = (doc: any) => {
    const collName = 'smoke_test_transactions';
    // On simule une modification
    const newDoc = { 
      ...doc, 
      name: `${doc.name} (edited)`, 
      status: 'published',
      updatedAt: new Date().toISOString()
    };

    txRef.current.update(collName, newDoc);
    addLog(`📝 Staged: UPDATE "${doc.id}"`);
    setPendingOps(txRef.current.getPendingOperations());
  }

  const stageDelete = (id: string) => {
    const collName = 'smoke_test_transactions';
    txRef.current.delete(collName, id);
    addLog(`📝 Staged: DELETE "${id}"`);
    setPendingOps(txRef.current.getPendingOperations());
  }

  // --- Actions Finales (COMMIT / ROLLBACK) ---

  const handleCommit = async () => {
    if (pendingOps.length === 0) return;
    
    try {
      addLog(`🚀 Committing ${pendingOps.length} operations...`);
      await txRef.current.commit();
      addLog(`✅ Transaction Committed (ACID) !`);
      
      setPendingOps([]); // Vide l'UI
      await refreshItems(); // Met à jour la vue réelle
      
    } catch (e: any) {
      addLog(`❌ Transaction Failed: ${e}`);
    }
  }

  const handleRollback = () => {
    txRef.current.rollback();
    setPendingOps([]);
    addLog(`↩️ Rollback (Annulation des modifications en attente)`);
  }

  // --- UI ---

  return (
    <div style={{ padding: 20, background: '#111827', borderRadius: 8, border: '1px solid #374151', marginTop: 20, height: '600px', display: 'flex', flexDirection: 'column' }}>
      <div style={{display:'flex', justifyContent:'space-between', alignItems:'center', marginBottom: 16}}>
        <h3 style={{ color: '#fff', margin: 0 }}>⚛️ Transaction Manager (ACID)</h3>
        <div style={{fontSize: '0.9em', color: '#9ca3af'}}>
          Collection: <code style={{color: '#e5e7eb'}}>smoke_test_transactions</code>
        </div>
      </div>
      
      <div style={{ display: 'grid', gridTemplateColumns: '350px 1fr', gap: 20, flex: 1, overflow: 'hidden' }}>
        
        {/* COLONNE GAUCHE : Staging Area */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          
          {/* Contrôles */}
          <div style={{ background: '#1f2937', padding: 12, borderRadius: 8 }}>
            <Button onClick={stageInsert} style={{width: '100%', marginBottom: 10}}>
              + Préparer une Création
            </Button>
            <div style={{ display: 'flex', gap: 8 }}>
              <Button 
                onClick={handleCommit} 
                disabled={pendingOps.length === 0}
                style={{ flex: 1, backgroundColor: pendingOps.length > 0 ? '#10b981' : '#374151' }}
              >
                Commit ({pendingOps.length})
              </Button>
              <Button 
                onClick={handleRollback} 
                disabled={pendingOps.length === 0}
                style={{ flex: 1, background: 'transparent', border: '1px solid #ef4444', color: '#ef4444', opacity: pendingOps.length === 0 ? 0.5 : 1 }}
              >
                Rollback
              </Button>
            </div>
          </div>

          {/* Liste des opérations en attente */}
          <div style={{ flex: 1, background: '#1f2937', borderRadius: 8, padding: 10, overflowY: 'auto' }}>
            <h4 style={{ color: '#9ca3af', marginTop: 0, fontSize: '0.9em', borderBottom: '1px solid #374151', paddingBottom: 8 }}>
              File d'attente (RAM)
            </h4>
            {pendingOps.length === 0 ? (
              <div style={{ color: '#6b7280', textAlign: 'center', padding: 20, fontSize: '0.85em', fontStyle: 'italic' }}>
                Aucune modification en attente.<br/>
                Ajoutez un élément ou modifiez la liste de droite.
              </div>
            ) : (
              <ul style={{ listStyle: 'none', padding: 0, margin: 0 }}>
                {pendingOps.map((op, i) => {
                  const style = OP_STYLES[op.type];
                  return (
                    <li key={i} style={{ fontSize: '0.85em', background: '#374151', marginBottom: 6, padding: 8, borderRadius: 4, display: 'flex', flexDirection: 'column' }}>
                      <div style={{display: 'flex', justifyContent: 'space-between', marginBottom: 4}}>
                        <span style={{ color: style.color, fontWeight: 'bold', fontSize: '0.8em' }}>{style.label}</span>
                        <span style={{ color: '#9ca3af', fontSize: '0.8em' }}>#{i+1}</span>
                      </div>
                      <div style={{ color: '#e5e7eb', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                        {op.type === 'delete' ? op.id : (op.doc.name || 'Sans nom')}
                      </div>
                    </li>
                  )
                })}
              </ul>
            )}
          </div>
          
          {/* Logs */}
          <div style={{ height: 120, background: '#000', padding: 8, borderRadius: 8, overflowY: 'auto', fontSize: '0.75em', fontFamily: 'monospace', color: '#4ade80' }}>
            {logs.map((l, i) => <div key={i}>{l}</div>)}
          </div>
        </div>

        {/* COLONNE DROITE : Données Réelles */}
        <div style={{ display: 'flex', flexDirection: 'column', background: '#1f2937', borderRadius: 8, overflow: 'hidden' }}>
          <div style={{ padding: 12, borderBottom: '1px solid #374151', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <h4 style={{ color: '#e5e7eb', margin: 0 }}>Données Réelles (Disque)</h4>
            <button onClick={refreshItems} style={{background:'none', border:'none', color:'#60a5fa', cursor:'pointer', fontSize: '0.9em'}}>
              ↻ Actualiser
            </button>
          </div>

          <div style={{ flex: 1, overflowY: 'auto', padding: 12 }}>
            {items.length === 0 ? (
              <div style={{ textAlign: 'center', color: '#6b7280', marginTop: 40 }}>
                La collection est vide.
              </div>
            ) : (
              <div style={{ display: 'grid', gap: 10 }}>
                {items.map((item) => (
                  <div key={item.id} style={{ background: '#111827', padding: 12, borderRadius: 6, border: '1px solid #374151', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <div style={{ overflow: 'hidden' }}>
                      <div style={{ color: '#f3f4f6', fontWeight: 500 }}>{item.name || 'Sans nom'}</div>
                      <div style={{ color: '#6b7280', fontSize: '0.8em', fontFamily: 'monospace' }}>ID: {item.id}</div>
                      <div style={{ display: 'flex', gap: 8, marginTop: 4 }}>
                        <span style={{ fontSize: '0.75em', background: '#374151', padding: '2px 6px', borderRadius: 4, color: '#d1d5db' }}>
                          {item.status || 'N/A'}
                        </span>
                      </div>
                    </div>
                    
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                      <button 
                        onClick={() => stageUpdate(item)}
                        style={{ border: '1px solid #3b82f6', background: 'transparent', color: '#60a5fa', padding: '4px 8px', borderRadius: 4, cursor: 'pointer', fontSize: '0.8em' }}
                      >
                        Modifier
                      </button>
                      <button 
                        onClick={() => stageDelete(item.id)}
                        style={{ border: '1px solid #ef4444', background: 'transparent', color: '#f87171', padding: '4px 8px', borderRadius: 4, cursor: 'pointer', fontSize: '0.8em' }}
                      >
                        Supprimer
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

      </div>
    </div>
  )
}