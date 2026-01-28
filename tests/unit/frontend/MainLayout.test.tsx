import React from 'react';
import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MainLayout } from '../../../src/components/layout/MainLayout';
import { useUiStore } from '../../../src/store/ui-store';

// 1. Correction : Ajout des parenthèses autour des arguments de vi.mock
vi.mock('../../../src/components/layout/Sidebar', () => ({
  Sidebar: () => <div data-testid="sidebar">Sidebar Mock</div>,
}));

vi.mock('../../../src/components/layout/Header', () => ({
  Header: ({ title }: { title: string }) => <div data-testid="header">{title}</div>,
}));

// 2. Mock du SpatialScene
vi.mock('../../../src/components/spatial/SpatialScene', () => ({
  SpatialScene: () => <div data-testid="spatial-scene-mock">Spatial Scene Mock</div>,
}));

// 3. Mock du Store
vi.mock('../../../src/store/ui-store', () => ({
  useUiStore: vi.fn(),
}));

describe('MainLayout Component', () => {
  const defaultProps = {
    currentPage: 'home',
    onNavigate: vi.fn(),
    pageTitle: 'Test Page',
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders correctly in 2D mode (Standard)', () => {
    // Simulation : Mode 2D, Sidebar ouverte
    (useUiStore as unknown as Mock).mockReturnValue({
      viewMode: '2d',
      sidebarOpen: true,
    });

    render(
      <MainLayout {...defaultProps}>
        <div data-testid="child-content">Contenu Principal</div>
      </MainLayout>,
    );

    expect(screen.getByTestId('sidebar')).toBeInTheDocument();
    expect(screen.getByTestId('header')).toHaveTextContent('Test Page');
    expect(screen.getByTestId('child-content')).toBeVisible();

    // Le moteur 3D NE DOIT PAS être là en mode 2D
    expect(screen.queryByTestId('spatial-scene-mock')).toBeNull();
  });

  it('renders the SpatialScene in 3D mode', () => {
    // Simulation : Mode 3D
    (useUiStore as unknown as Mock).mockReturnValue({
      viewMode: '3d',
      sidebarOpen: true,
    });

    render(
      <MainLayout {...defaultProps}>
        <div>Contenu</div>
      </MainLayout>,
    );

    // Vérification : La scène 3D DOIT être présente
    expect(screen.getByTestId('spatial-scene-mock')).toBeInTheDocument();

    // Vérification : Le container 3D doit être visible
    const container = screen.getByLabelText('Scène 3D');
    expect(container).toBeInTheDocument();
  });

  it('renders the SpatialScene in Hybrid mode', () => {
    // Simulation : Mode Hybride
    (useUiStore as unknown as Mock).mockReturnValue({
      viewMode: 'hybrid',
      sidebarOpen: false,
    });

    render(
      <MainLayout {...defaultProps}>
        <div data-testid="child-hybrid">Contenu Hybride</div>
      </MainLayout>,
    );

    // En hybride, on veut la 3D ET le contenu
    expect(screen.getByTestId('spatial-scene-mock')).toBeInTheDocument();
    expect(screen.getByTestId('child-hybrid')).toBeInTheDocument();
  });
});
