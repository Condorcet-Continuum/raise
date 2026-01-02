# Module Model Viewer (Explorateur Arcadia) 💠

Ce module est le cœur fonctionnel de RAISE pour la visualisation des modèles d'architecture système (Capella / Arcadia).
Il offre une interface riche et dense ("Data-Heavy") permettant de naviguer dans les arborescences complexes tout en visualisant les diagrammes associés.

---

## 📂 Structure du dossier

| Fichier                 | Rôle                                                                                                                          |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| **`CapellaViewer.tsx`** | **Composant Maître**. Orchestre la disposition globale en 3 colonnes via `SplitPane` (Navigateur \| Diagramme \| Inspecteur). |
| `ArcadiaLayerView.tsx`  | Barre latérale compacte permettant de basculer entre les couches d'abstraction (OA, SA, LA, PA, EPBS).                        |
| `ModelNavigator.tsx`    | Arbre de navigation (`TreeView`) pour explorer la hiérarchie des éléments du modèle.                                          |
| `DiagramRenderer.tsx`   | Zone centrale d'affichage des diagrammes (simulée pour l'instant, prête pour le rendu SVG/Canvas).                            |
| `ElementInspector.tsx`  | Panneau de propriétés (droite) affichant les détails de l'élément sélectionné.                                                |
| `DataDictionary.tsx`    | Vue alternative sous forme de liste catégorisée par type d'élément (Acteurs, Fonctions, Composants).                          |

---

## 🎨 Design & Thèmes

Ce module respecte le code couleur standard de la méthode **Arcadia** tout en s'intégrant au thème global (Light/Dark).

### Code Couleur Arcadia :

Les couches sont identifiées par des couleurs spécifiques, utilisées dans la navigation et les bordures :

- 🟠 **OA (Operational Analysis) :** Orange (`#f59e0b`)
- 🟢 **SA (System Analysis) :** Vert (`#10b981`)
- 🔵 **LA (Logical Architecture) :** Bleu (`#3b82f6`)
- 🟣 **PA (Physical Architecture) :** Violet (`#8b5cf6`)
- 🔴 **EPBS (End Product) :** Rose/Rouge (`#db2777`)

### Adaptation chromatique :

- **Structure :** Utilise `var(--bg-app)` pour le fond global et `var(--bg-panel)` pour les panneaux (navigateur, inspecteur), créant une hiérarchie visuelle claire.
- **Séparateurs :** Les composants `SplitPane` et les bordures utilisent `var(--border-color)` pour rester discrets quel que soit le mode.
- **Texte :** Hiérarchie stricte entre `var(--text-main)` (contenu) et `var(--text-muted)` (labels, métadonnées).

---

## 💻 Fonctionnalités

1.  **Layout Resizable :**

    - Utilisation du composant `SplitPane` (dans `src/components/shared`) pour permettre à l'utilisateur de redimensionner les colonnes (Navigateur/Diagramme/Inspecteur) selon ses besoins.

2.  **Filtrage par Couche (Layering) :**

    - Le composant `ArcadiaLayerView` agit comme un filtre global. Sélectionner "LA" (Logical Architecture) ne montre que les diagrammes et éléments logiques.

3.  **Inspection Contextuelle :**
    - Cliquer sur un élément dans l'arbre ou le diagramme met à jour le panneau `ElementInspector` à droite.

---

## 💻 Exemple d'intégration

Le `CapellaViewer` est conçu pour être une page à part entière.

```tsx
import CapellaViewer from '@/components/model-viewer/CapellaViewer';

export default function ModelPage() {
  // Le viewer gère sa propre hauteur (100%)
  return (
    <div style={{ height: 'calc(100vh - 64px)' }}>
      <CapellaViewer />
    </div>
  );
}
```

---

## 🛠️ Évolutions possibles

- **Rendu SVG Réel :** Remplacer le placeholder du `DiagramRenderer` par une librairie de rendu vectoriel capable de lire les fichiers `.aird`(ou un format exporté JSON).
- **Recherche Globale :** Ajouter une barre de recherche dans le `ModelNavigator` pour filtrer l'arbre.
- **Breadcrumbs :** Ajouter un fil d'ariane pour savoir où l'on se situe dans la profondeur du modèle.
