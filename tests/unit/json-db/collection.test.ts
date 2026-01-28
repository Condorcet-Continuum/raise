import { describe, it, expect, vi, beforeEach } from 'vitest';
import { collectionService } from '../../../src/services/json-db/collection-service'; // Adaptez le chemin si besoin

// Mock de tauri invoke
const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  // CORRECTION : On remplace 'any' par un type plus strict
  invoke: (cmd: string, args: Record<string, unknown>) => invokeMock(cmd, args),
}));

describe('Collection Service', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('creates a collection successfully', async () => {
    invokeMock.mockResolvedValue(undefined);

    // CORRECTION 1 : On passe le schéma en string via JSON.stringify
    // CORRECTION 2 : On n'attend pas de valeur de retour (void)
    await collectionService.createCollection(
      'test_collection',
      JSON.stringify({ type: 'object', properties: { name: { type: 'string' } } }),
    );

    expect(invokeMock).toHaveBeenCalledWith('jsondb_create_collection', {
      name: 'test_collection',
      schema: JSON.stringify({ type: 'object', properties: { name: { type: 'string' } } }),
    });
  });
});
