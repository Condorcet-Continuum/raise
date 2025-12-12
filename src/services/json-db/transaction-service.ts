import { invoke } from '@tauri-apps/api/core';
import { useSettingsStore } from '@/store/settings-store';
import type { OperationRequest } from '@/types/json-db.types';
import { v4 as uuidv4 } from 'uuid';

export class TransactionService {
  private operations: OperationRequest[] = [];
  private space: string;
  private db: string;

  constructor(space?: string, db?: string) {
    // Si non fourni, on prend la config globale actuelle
    const config = useSettingsStore.getState();
    this.space = space || config.jsonDbSpace;
    this.db = db || config.jsonDbDatabase;
  }

  add(collection: string, doc: Record<string, any>): this {
    const id = doc.id || uuidv4();
    const docWithId = { ...doc, id };

    this.operations.push({
      type: 'Insert',
      collection,
      id,
      document: docWithId,
    });
    return this;
  }

  update(collection: string, id: string, doc: Record<string, any>): this {
    this.operations.push({
      type: 'Update',
      collection,
      id,
      document: doc,
    });
    return this;
  }

  delete(collection: string, id: string): this {
    this.operations.push({
      type: 'Delete',
      collection,
      id,
    });
    return this;
  }

  getPendingOperations(): OperationRequest[] {
    return [...this.operations];
  }

  rollback(): void {
    this.operations = [];
  }

  async commit(): Promise<void> {
    if (this.operations.length === 0) return;

    // Simulation de transaction séquentielle
    // (Dans une vraie DB, ce serait un appel batch atomique)
    for (const op of this.operations) {
      try {
        if (op.type === 'Insert') {
          await invoke('jsondb_insert_document', {
            space: this.space,
            db: this.db,
            collection: op.collection,
            document: op.document,
          });
        } else if (op.type === 'Update') {
          await invoke('jsondb_update_document', {
            space: this.space,
            db: this.db,
            collection: op.collection,
            id: op.id,
            document: op.document,
          });
        } else if (op.type === 'Delete') {
          await invoke('jsondb_delete_document', {
            space: this.space,
            db: this.db,
            collection: op.collection,
            id: op.id,
          });
        }
      } catch (error) {
        console.error(`[Transaction] Error on ${op.type}:`, error);
        // On pourrait throw ici pour arrêter la séquence
        throw error;
      }
    }

    this.operations = [];
  }
}

export const createTransaction = (space?: string, db?: string) => new TransactionService(space, db);
