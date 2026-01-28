// src/types/ui.types.ts

export type ThemeMode = 'light' | 'dark' | 'system';
export type ViewMode = '2d' | '3d' | 'hybrid';
export type PanelLayout = 'single' | 'split';

export interface CameraState {
  position: [number, number, number];
  target: [number, number, number];
  zoom: number;
}

export interface SpatialSelection {
  elementId: string | null;
  domain?: string;
}

/**
 * État pur des données du store UI
 */
export interface UiState {
  theme: ThemeMode;
  sidebarOpen: boolean;
  panelLayout: PanelLayout;
  viewMode: ViewMode;
  cameraState: CameraState;
  selection: SpatialSelection;
}

/**
 * Actions disponibles pour modifier l'état
 */
export interface UiActions {
  setTheme: (mode: ThemeMode) => void;
  toggleSidebar: () => void;
  setPanelLayout: (layout: PanelLayout) => void;
  setViewMode: (mode: ViewMode) => void;
  setCameraState: (state: Partial<CameraState>) => void;
  setSelection: (elementId: string | null, domain?: string) => void;
  resetCamera: () => void;
}

// Type combiné pour le store
export type UiStoreState = UiState & UiActions;
