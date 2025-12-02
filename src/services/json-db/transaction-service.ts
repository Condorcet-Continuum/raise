import { invoke } from '@tauri-apps/api/core';
import type { OperationRequest } from '@/types/json-db.types';
// On utilise uuid pour générer un ID temporaire si besoin
import { v4 as uuidv4 } from 'uuid';

const DEFAULT_SPACE = 'un2';
const DEFAULT_DB = '_system';

export class TransactionService {
  private operations: OperationRequest[] = [];

  constructor(private space: string = DEFAULT_SPACE, private db: string = DEFAULT_DB) {}

  add(collection: string, doc: Record<string, any>): this {
    // Si le doc n'a pas d'ID, on en génère un pour le frontend
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

  // CORRECTION ICI : Signature à 3 arguments pour correspondre à JsonDbTester
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

    // Exécution séquentielle des opérations (simulation de transaction)
    for (const op of this.operations) {
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
    }

    this.operations = [];
  }
}

export const createTransaction = (space?: string, db?: string) => new TransactionService(space, db);
