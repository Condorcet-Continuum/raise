import { useModelStore } from '@/store/model-store';

/**
 * Facade pour accéder aux données du modèle Arcadia/SysML actif.
 * Simplifie l'accès au store depuis les composants UI.
 */
export function useModelState() {
  const { project, elementsById, selectedElementId, setProject, selectElement, updateElement } =
    useModelStore();

  // Calcul dérivé : l'objet complet de l'élément sélectionné
  const selectedElement =
    selectedElementId && elementsById[selectedElementId]
      ? elementsById[selectedElementId]
      : undefined;

  return {
    // Données
    project,
    hasProject: !!project,
    projectName: project?.name || 'Sans titre',

    // Sélection
    selectedElementId,
    selectedElement,

    // Actions
    setProject,
    selectElement,
    updateElement,

    // Helpers d'accès direct
    getElementById: (id: string) => elementsById[id],
  };
}
