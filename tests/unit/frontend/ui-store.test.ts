// tests/unit/frontend/ui-store.test.ts

import { describe, it, expect, beforeEach, vi } from 'vitest';
// Utilisation du chemin relatif pour respecter le patrimoine existant
import { useUiStore } from '../../../src/store/ui-store';

describe('UiStore', () => {
  beforeEach(() => {
    // Initialisation propre avant chaque test
    const state = useUiStore.getState();
    state.resetCamera();
    state.setViewMode('2d');
    state.setTheme('system');

    // Mock de localStorage si nécessaire pour l'environnement de test
    vi.clearAllMocks();
  });

  it('should initialize with default values', () => {
    const state = useUiStore.getState();
    expect(state.viewMode).toBe('2d');
    expect(state.theme).toBe('system');
    expect(state.cameraState.position).toEqual([10, 10, 10]);
  });

  it('should update view mode', () => {
    const { setViewMode } = useUiStore.getState();
    setViewMode('3d');
    expect(useUiStore.getState().viewMode).toBe('3d');
  });

  it('should handle spatial selection', () => {
    const { setSelection } = useUiStore.getState();
    const uuid = '550e8400-e29b-41d4-a716-446655440000';

    setSelection(uuid, 'arcadia');
    const { selection } = useUiStore.getState();

    expect(selection.elementId).toBe(uuid);
    expect(selection.domain).toBe('arcadia');
  });

  it('should partial update camera state without losing other coordinates', () => {
    const { setCameraState } = useUiStore.getState();
    setCameraState({ zoom: 5 });

    const state = useUiStore.getState();
    expect(state.cameraState.zoom).toBe(5);
    expect(state.cameraState.position).toEqual([10, 10, 10]);
  });
});
