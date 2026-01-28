// src/store/ui-store.ts

import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import { UiStoreState, CameraState } from '../types/ui.types'; // Import relatif pour éviter les problèmes d'alias

const DEFAULT_CAMERA: CameraState = {
  position: [10, 10, 10],
  target: [0, 0, 0],
  zoom: 1,
};

export const useUiStore = create<UiStoreState>()(
  persist(
    (set) => ({
      // --- ÉTAT INITIAL ---
      theme: 'system',
      sidebarOpen: true,
      panelLayout: 'split',
      viewMode: '2d',
      cameraState: DEFAULT_CAMERA,
      selection: { elementId: null },

      // --- ACTIONS ---
      setTheme: (theme) => set({ theme }),
      toggleSidebar: () => set((state) => ({ sidebarOpen: !state.sidebarOpen })),
      setPanelLayout: (panelLayout) => set({ panelLayout }),
      setViewMode: (viewMode) => set({ viewMode }),
      setCameraState: (coords) =>
        set((state) => ({
          cameraState: { ...state.cameraState, ...coords },
        })),
      setSelection: (elementId, domain) => set({ selection: { elementId, domain } }),
      resetCamera: () => set({ cameraState: DEFAULT_CAMERA }),
    }),
    {
      name: 'raise-ui-storage',
      storage: createJSONStorage(() => localStorage),
      // On persiste les préférences et l'état de la caméra
      partialize: (state) => ({
        theme: state.theme,
        sidebarOpen: state.sidebarOpen,
        panelLayout: state.panelLayout,
        viewMode: state.viewMode,
        cameraState: state.cameraState,
      }),
    },
  ),
);
