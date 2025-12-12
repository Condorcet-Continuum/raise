import { useModelStore } from '@/store/model-store';
import { TreeView, TreeNode } from '@/components/shared/TreeView';
import type { ProjectModel, ArcadiaElement } from '@/types/model.types';

export function ModelNavigator() {
  const { project, selectElement } = useModelStore();

  // Fonction pour transformer une liste d'éléments en nœuds d'arbre
  const mapElements = (elements: ArcadiaElement[] | undefined, icon: string): TreeNode[] => {
    if (!elements || elements.length === 0) return [];
    return elements.map((el) => ({
      id: el.id,
      label: el.name as string, // Cast simple pour la démo
      icon,
    }));
  };

  // Construction de l'arbre sémantique Arcadia
  const buildArcadiaTree = (proj: ProjectModel): TreeNode[] => {
    const rootNodes: TreeNode[] = [];

    // Helper pour créer un dossier de couche
    const createLayerNode = (
      id: string,
      label: string,
      color: string,
      content?: { label: string; items: ArcadiaElement[]; icon: string }[],
    ): TreeNode | null => {
      if (!content) return null;

      // On filtre les catégories vides
      const children = content.flatMap((cat) => {
        const items = mapElements(cat.items, cat.icon);
        if (items.length === 0) return [];
        return [
          {
            id: `${id}-${cat.label}`,
            label: `${cat.label} (${items.length})`,
            children: items,
            icon: '📂',
          },
        ];
      });

      if (children.length === 0) return null;

      return {
        id,
        label: <span style={{ fontWeight: 'bold', color }}>{label}</span>,
        children,
        isExpanded: true, // Par défaut déplié
      };
    };

    // 1. Analyse Opérationnelle (Orange)
    if (proj.oa) {
      rootNodes.push(
        createLayerNode('oa', 'Operational Analysis', '#f59e0b', [
          { label: 'Operational Capabilities', items: proj.oa.capabilities, icon: '⚡' },
          { label: 'Operational Activities', items: proj.oa.activities, icon: '⚙️' },
          { label: 'Operational Actors', items: proj.oa.actors, icon: '👤' },
        ])!,
      );
    }

    // 2. Analyse Système (Vert)
    if (proj.sa) {
      rootNodes.push(
        createLayerNode('sa', 'System Analysis', '#10b981', [
          { label: 'System Capabilities', items: proj.sa.capabilities, icon: '⚡' },
          { label: 'System Functions', items: proj.sa.functions, icon: 'ƒ' },
          { label: 'System Components', items: proj.sa.components, icon: '📦' },
        ])!,
      );
    }

    // 3. Architecture Logique (Bleu)
    if (proj.la) {
      rootNodes.push(
        createLayerNode('la', 'Logical Architecture', '#3b82f6', [
          { label: 'Logical Functions', items: proj.la.functions, icon: 'ƒ' },
          { label: 'Logical Components', items: proj.la.components, icon: '🧩' },
        ])!,
      );
    }

    // 4. Architecture Physique (Violet)
    if (proj.pa) {
      rootNodes.push(
        createLayerNode('pa', 'Physical Architecture', '#8b5cf6', [
          { label: 'Physical Functions', items: proj.pa.functions, icon: 'ƒ' },
          { label: 'Physical Components', items: proj.pa.components, icon: '🖥️' },
        ])!,
      );
    }

    return rootNodes.filter(Boolean); // Nettoyage des nulls
  };

  const nodes = project ? buildArcadiaTree(project) : [];

  return (
    <div
      style={{
        height: '100%',
        overflowY: 'auto',
        padding: 'var(--spacing-2)',
        backgroundColor: 'var(--bg-panel)',
        color: 'var(--text-main)',
        fontFamily: 'var(--font-family)',
      }}
    >
      <div
        style={{
          padding: 'var(--spacing-2)',
          borderBottom: '1px solid var(--border-color)',
          marginBottom: 'var(--spacing-2)',
          fontWeight: 'bold',
          fontSize: 'var(--font-size-sm)',
          color: 'var(--text-muted)',
          textTransform: 'uppercase',
        }}
      >
        Explorateur de Projet
      </div>

      {project ? (
        <TreeView nodes={nodes} onSelect={(id) => selectElement(id)} />
      ) : (
        <div
          style={{
            padding: 'var(--spacing-4)',
            color: 'var(--text-muted)',
            fontStyle: 'italic',
            fontSize: 'var(--font-size-sm)',
          }}
        >
          Aucun modèle chargé.
        </div>
      )}
    </div>
  );
}
