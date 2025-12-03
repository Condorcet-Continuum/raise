// FICHIER : src/store/model-store.ts

import { create } from 'zustand';
import type { ProjectModel, ArcadiaElement } from '@/types/model.types';

export interface ModelStoreState {
  // État
  project: ProjectModel | null;
  isLoading: boolean;
  error: string | null;

  // Indexation rapide & Sélection
  elementsById: Record<string, ArcadiaElement>;
  selectedElementId?: string | null; // Ajouté pour la compatibilité UI

  // Actions
  setProject: (model: ProjectModel) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  selectElement: (id: string | null | undefined) => void; // Ajouté

  // Helpers
  getElementById: (id: string) => ArcadiaElement | undefined;
}

export const useModelStore = create<ModelStoreState>((set, get) => ({
  project: null,
  isLoading: false,
  error: null,
  elementsById: {},
  selectedElementId: null,

  setProject: (model) => {
    // On indexe tout à plat pour les recherches rapides O(1)
    const map: Record<string, ArcadiaElement> = {};

    // Helper pour indexer une liste
    const indexList = (list?: ArcadiaElement[]) => {
      if (!list) return;
      list.forEach((el) => {
        map[el.id] = el;
      });
    };

    // Indexation des couches si elles existent
    if (model.oa) {
      indexList(model.oa.actors);
      indexList(model.oa.activities);
      indexList(model.oa.capabilities);
    }
    if (model.sa) {
      indexList(model.sa.functions);
      indexList(model.sa.components);
      indexList(model.sa.actors);
    }
    // ... (Ajouter LA, PA, EPBS si nécessaire)

    set({ project: model, elementsById: map, error: null });
  },

  setLoading: (isLoading) => set({ isLoading }),

  setError: (error) => set({ error }),

  selectElement: (id) => set({ selectedElementId: id || null }),

  getElementById: (id) => get().elementsById[id],
}));
