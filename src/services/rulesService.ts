// FICHIER : src/services/rulesService.ts

import { invoke } from '@tauri-apps/api/core';
import { Rule, ValidationIssue, JsonValue } from '../types/rules.types';

/**
 * Teste une règle unitaire sans la sauvegarder.
 * [LINT FIX] Typage strict des entrées/sorties
 */
export const dryRunRule = async (
  rule: Rule,
  context: Record<string, unknown>,
): Promise<JsonValue> => {
  try {
    return await invoke('dry_run_rule', { rule, context });
  } catch (error) {
    console.error('Erreur Dry Run:', error);
    throw error;
  }
};

/**
 * Lance la validation complète du modèle.
 */
export const validateModel = async (rules: Rule[]): Promise<ValidationIssue[]> => {
  try {
    return await invoke('validate_model', { rules });
  } catch (error) {
    console.error('Erreur Validation:', error);
    throw error;
  }
};
