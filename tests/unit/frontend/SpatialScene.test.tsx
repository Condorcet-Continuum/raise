// FICHIER : tests/unit/frontend/SpatialScene.test.tsx

import React from 'react';
import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SpatialScene } from '../../../src/components/spatial/SpatialScene';
import { spatialService } from '../../../src/services/spatial-service';
import { useUiStore } from '../../../src/store/ui-store';

// Définition des types pour le Mock
interface MockCanvasProps {
  children: React.ReactNode;
  onPointerMissed?: () => void;
}

// 1. Mock de React Three Fiber corrigé (Typage strict)
vi.mock('@react-three/fiber', () => ({
  Canvas: ({ children, onPointerMissed }: MockCanvasProps) => (
    <div data-testid="canvas-mock" onClick={() => onPointerMissed && onPointerMissed()}>
      {children}
    </div>
  ),
}));

// 2. Mock des composants Drei
vi.mock('@react-three/drei', () => ({
  OrbitControls: () => null,
  Stars: () => null,
  Text: () => null,
}));

// 3. Mock du Service et du Store
vi.mock('../../../src/services/spatial-service');
vi.mock('../../../src/store/ui-store');

describe('SpatialScene Component', () => {
  const setSelectionMock = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();

    // Simulation du Store Zustand
    (useUiStore as unknown as Mock).mockImplementation((selector) => {
      const state = {
        selection: { elementId: null },
        setSelection: setSelectionMock,
      };
      return selector ? selector(state) : state;
    });
  });

  it('renders correctly and removes loading state', async () => {
    const mockData = {
      nodes: [
        { id: 'n1', label: 'Node 1', position: [0, 0, 0], layer: 0, weight: 1, stability: 1 },
      ],
      links: [],
      meta: { node_count: 1, layer_distribution: [0, 0, 0, 0, 0] },
    };
    // On doit caster le positionnement pour satisfaire TypeScript si nécessaire, ou laisser l'inférence
    (spatialService.getTopology as Mock).mockResolvedValue(mockData);

    render(<SpatialScene />);

    await waitFor(() => {
      expect(screen.queryByText(/INITIALIZING/i)).toBeNull();
    });
    expect(screen.getByTestId('canvas-mock')).toBeInTheDocument();
  });

  it('selects a node when clicked', async () => {
    const mockData = {
      nodes: [
        {
          id: 'target-node',
          label: 'Target',
          position: [0, 0, 0],
          layer: 0,
          weight: 1,
          stability: 1,
        },
      ],
      links: [],
      meta: { node_count: 1, layer_distribution: [0, 0, 0, 0, 0] },
    };
    (spatialService.getTopology as Mock).mockResolvedValue(mockData);

    const { container } = render(<SpatialScene />);
    await waitFor(() => expect(screen.queryByText(/INITIALIZING/i)).toBeNull());

    // Le <mesh> est transformé en balise HTML par le moteur de test
    const nodeMesh = container.querySelector('mesh');
    expect(nodeMesh).toBeInTheDocument();

    if (nodeMesh) {
      fireEvent.click(nodeMesh);
    }

    expect(setSelectionMock).toHaveBeenCalledWith('target-node', 'spatial');
  });

  it('deselects when clicking on empty space (background)', async () => {
    const mockData = {
      nodes: [],
      links: [],
      meta: { node_count: 0, layer_distribution: [0, 0, 0, 0, 0] },
    };
    (spatialService.getTopology as Mock).mockResolvedValue(mockData);

    render(<SpatialScene />);
    await waitFor(() => expect(screen.queryByText(/INITIALIZING/i)).toBeNull());

    // Clic sur le Canvas (le fond) déclenche onPointerMissed via le Mock
    const canvas = screen.getByTestId('canvas-mock');
    fireEvent.click(canvas);

    expect(setSelectionMock).toHaveBeenCalledWith(null, undefined);
  });
});
