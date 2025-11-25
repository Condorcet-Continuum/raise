// src/services/json-db/transaction-service.ts
import { invoke } from '@tauri-apps/api/core';
import type { OperationRequest, TransactionRequest } from '@/types/json-db.types';

const DEFAULT_SPACE = 'un2';
const DEFAULT_DB = '_system';

export class TransactionService {
  private operations: OperationRequest[] = [];

  constructor(
    private space: string = DEFAULT_SPACE, 
    private db: string = DEFAULT_DB
  ) {}

  /**
   * Ajoute une opération d'insertion à la transaction.
   * L'ID sera généré par le backend s'il est absent du doc.
   */
  add(collection: string, doc: Record<string, any>): this {
    this.operations.push({ type: 'insert', collection, doc });
    return this;
  }

  /**
   * Ajoute une opération de mise à jour.
   * Le document DOIT contenir un champ 'id'.
   */
  update(collection: string, doc: Record<string, any>): this {
    this.operations.push({ type: 'update', collection, doc });
    return this;
  }

  /**
   * Ajoute une opération de suppression.
   */
  delete(collection: string, id: string): this {
    this.operations.push({ type: 'delete', collection, id });
    return this;
  }

/**
   * Retourne la liste des opérations en attente (pour affichage UI).
   */
getPendingOperations(): OperationRequest[] {
    return [...this.operations]; // Retourne une copie pour éviter la mutation externe
  }

  /**
   * Annule toutes les opérations en attente (Rollback Client).
   */
  rollback(): void {
    this.operations = [];
  }
    
  /**
   * Exécute la transaction de manière atomique.
   */
  async commit(): Promise<void> {
    if (this.operations.length === 0) return;

    const request: TransactionRequest = {
      operations: this.operations
    };

    try {
      await invoke('jsondb_execute_transaction', {
        space: this.space,
        db: this.db,
        request
      });
      // Reset après succès
      this.operations = [];
    } catch (error) {
      console.error("Transaction failed:", error);
      throw error; // Relancer pour que l'UI puisse gérer l'erreur
    }
  }
}

// Helper pour créer une nouvelle transaction rapidement
export const createTransaction = (space?: string, db?: string) => 
  new TransactionService(space, db);