// FICHIER : src/hooks/useModelState.ts

import { useModelStore } from '@/store/model-store';

export function useModelState() {
  const {
    project, // Remplaçant de currentModelId
    elementsById,
    selectedElementId,
    setProject, // Remplaçant de setElements / setCurrentModel
    selectElement,
  } = useModelStore();

  const selectedElement =
    selectedElementId && elementsById[selectedElementId]
      ? elementsById[selectedElementId]
      : undefined;

  return {
    // On expose le projet complet
    project,
    // Rétro-compatibilité : si l'UI a besoin d'un ID, on peut renvoyer un ID fictif ou l'ID du projet si dispo
    currentModelId: project ? 'loaded-project' : undefined,

    elementsById,
    selectedElementId,
    selectedElement,

    // Actions mises à jour
    setProject,
    // Alias pour la compatibilité si d'autres composants appellent setElements
    setElements: () => console.warn('setElements is deprecated, use setProject via ModelService'),
    selectElement,
  };
}
