import { create } from 'zustand';
// ✅ IMPORTATION DES TYPES CENTRALISÉS (Crucial pour éviter l'erreur)
import type { ProjectModel, ArcadiaElement } from '@/types/model.types';

export interface ModelStoreState {
  // État
  project: ProjectModel | null;
  isLoading: boolean;
  error: string | null;

  // Indexation & Sélection
  elementsById: Record<string, ArcadiaElement>;
  selectedElementId: string | null;

  // Actions
  setProject: (model: ProjectModel) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  selectElement: (id: string | null | undefined) => void;
  updateElement: (id: string, data: Partial<ArcadiaElement>) => void;

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
    // Indexation à plat pour accès O(1)
    const map: Record<string, ArcadiaElement> = {};

    if (model.id) {
      map[model.id] = model as unknown as ArcadiaElement;
    }

    // Helper d'indexation
    const indexList = (list?: ArcadiaElement[]) => {
      if (!list) return;
      list.forEach((el) => {
        if (el && el.id) map[el.id] = el;
      });
    };

    // Indexation des couches principales
    if (model.oa) {
      indexList(model.oa.actors);
      indexList(model.oa.activities);
      indexList(model.oa.capabilities);
    }
    if (model.sa) {
      indexList(model.sa.actors);
      indexList(model.sa.functions);
      indexList(model.sa.components);
    }
    if (model.la) {
      indexList(model.la.functions);
      indexList(model.la.components);
    }
    if (model.pa) {
      indexList(model.pa.components);
    }
    if (model.epbs) {
      indexList(model.epbs.configurationItems);
    }
    // Indexation de la couche Data (nouveau)
    if (model.data) {
      indexList(model.data.classes);
      indexList(model.data.dataTypes);
    }

    set({ project: model, elementsById: map, error: null, selectedElementId: null });
  },

  setLoading: (isLoading) => set({ isLoading }),

  setError: (error) => set({ error }),

  selectElement: (id) => set({ selectedElementId: id || null }),

  updateElement: (id, data) =>
    set((state) => {
      const existing = state.elementsById[id];
      if (!existing) return state;

      const updated = { ...existing, ...data };
      const newElementsById = { ...state.elementsById, [id]: updated };

      let newProject = state.project;
      if (state.project && state.project.id === id) {
        newProject = { ...state.project, ...data } as ProjectModel;
      }

      return {
        elementsById: newElementsById,
        project: newProject,
      };
    }),

  getElementById: (id) => get().elementsById[id],
}));
