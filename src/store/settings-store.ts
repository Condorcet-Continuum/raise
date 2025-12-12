import { create } from 'zustand';

export type AiBackend = 'mock' | 'tauri-local' | 'remote-api';

export interface SettingsState {
  language: 'fr' | 'en';
  aiBackend: AiBackend;

  // Configuration pour la base de données JSON
  jsonDbSpace: string;
  jsonDbDatabase: string;

  update: (partial: Partial<SettingsState>) => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  language: 'fr',
  aiBackend: 'mock', // Par défaut sur mock pour le dév UI sans backend
  jsonDbSpace: 'un2',
  jsonDbDatabase: '_system',

  update: (partial) => set((state) => ({ ...state, ...partial })),
}));
