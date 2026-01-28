// src/components/layout/MainLayout.tsx

import { ReactNode } from 'react';
import { Sidebar } from './Sidebar';
import { Header } from './Header';
import { useUiStore } from '../../store/ui-store';
import { SpatialScene } from '../spatial/SpatialScene';

interface MainLayoutProps {
  children: ReactNode;
  currentPage: string;
  onNavigate: (page: string) => void;
  pageTitle: string;
}

export function MainLayout({ children, currentPage, onNavigate, pageTitle }: MainLayoutProps) {
  const { viewMode, sidebarOpen } = useUiStore();

  // 1. STYLE CONTENEUR GLOBAL : Flex ROW pour mettre Sidebar et Contenu côte à côte
  const containerStyle: React.CSSProperties = {
    display: 'flex', // OBLIGATOIRE pour l'alignement horizontal
    flexDirection: 'row', // Explicite : gauche vers droite
    height: '100vh', // Prend toute la hauteur de l'écran
    width: '100vw', // Prend toute la largeur
    overflow: 'hidden', // Pas de scroll sur le body
    backgroundColor: 'var(--bg-app)',
    color: 'var(--text-main)',
  };

  // 2. STYLE COLONNE DE DROITE (Header + Main)
  const contentColumnStyle: React.CSSProperties = {
    display: 'flex',
    flexDirection: 'column', // Vertical : Header au-dessus du Main
    flex: 1, // Prend tout l'espace restant à droite
    height: '100%',
    minWidth: 0, // Important pour le truncate du texte
  };

  return (
    <div style={containerStyle}>
      {/* GAUCHE : Sidebar */}
      {sidebarOpen && <Sidebar currentPage={currentPage} onNavigate={onNavigate} />}

      {/* DROITE : Header + Contenu */}
      <div style={contentColumnStyle}>
        {/* HAUT : Header (taille fixe gérée dans Header.tsx) */}
        <Header title={pageTitle} />

        {/* BAS : Zone de contenu hybride */}
        <main style={{ position: 'relative', flex: 1, overflow: 'hidden' }}>
          {/* A. LAYER 3D (Arrière-plan) */}
          {(viewMode === '3d' || viewMode === 'hybrid') && (
            <div
              id="spatial-canvas-container"
              style={{
                position: 'absolute',
                inset: 0, // top:0, right:0, bottom:0, left:0
                width: '100%',
                height: '100%',
                zIndex: 0,
                backgroundColor: 'black',
              }}
              aria-label="Scène 3D"
            >
              <SpatialScene />
            </div>
          )}

          {/* B. LAYER 2D (Interface utilisateur) */}
          <div
            style={{
              position: 'relative',
              zIndex: 10,
              height: '100%',
              width: '100%',
              pointerEvents: viewMode === '3d' ? 'none' : 'auto',
              overflowY: viewMode === '3d' ? 'hidden' : 'auto',
            }}
          >
            {/* Contenu avec transition d'opacité */}
            <div
              style={{
                height: '100%',
                opacity: viewMode === '3d' ? 0 : 1,
                transition: 'opacity 0.3s ease',
              }}
            >
              {children}
            </div>
          </div>
        </main>
      </div>
    </div>
  );
}
