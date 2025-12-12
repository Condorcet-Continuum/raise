# Module Diagram Editor ✏️

Ce module fournit un environnement de modélisation visuelle complet (canvas infini).
Il permet aux architectes systèmes de créer et manipuler des diagrammes (SysML, Arcadia) via une interface "Drag & Drop".
L'éditeur est conçu pour être performant (CSS pur pour la grille) et parfaitement intégré au thème global.

---

## 📂 Structure du dossier

| Fichier                 | Rôle                                                                                                                  |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------- |
| **`DiagramCanvas.tsx`** | **Composant Maître**. Gère la zone de dessin, l'état des nœuds déposés, le Drag & Drop et le rendu du fond quadrillé. |
| `ShapeLibrary.tsx`      | Barre latérale (Sidebar) contenant les éléments graphiques (Blocs, Acteurs, etc.) prêts à être glissés.               |
| `ConnectionTool.tsx`    | Barre d'outils flottante (Floating Toolbar) pour changer de mode (Sélection, Lien, Texte).                            |
| `LayoutEngine.tsx`      | Panneau de contrôle pour déclencher les algorithmes de réorganisation automatique (Auto-layout).                      |

---

## 🎨 Design & Thèmes

L'éditeur s'adapte dynamiquement au mode Sombre/Clair, ce qui est crucial pour un outil utilisé sur de longues sessions.

### Adaptation chromatique :

- **Le Canvas (Fond) :** Utilise `var(--bg-app)`.
- **La Grille :** Générée en CSS pur (`linear-gradient`) avec la couleur `var(--text-main)` et une opacité très faible (0.1). Cela garantit que la grille est toujours visible mais discrète, que le fond soit blanc ou noir.
- **Les Nœuds (Formes) :**
  - Fond : `var(--bg-panel)`.
  - Bordure : `var(--color-primary)`.
  - Texte : `var(--text-main)`.
  - Ombre : `var(--shadow-md)` pour donner de la profondeur.
- **Outils Flottants :** Utilisent `var(--z-index-sticky)` pour rester au-dessus du dessin, avec un fond semi-transparent ou solide selon le composant.

---

## 💻 Fonctionnalités

1.  **Drag & Drop Natif :**

    - Utilise l'API HTML5 Drag & Drop (`draggable`, `onDragStart`, `onDrop`).
    - Transfert de données via `dataTransfer.setData('shapeType', ...)` depuis la `ShapeLibrary`.

2.  **Grid System CSS :**

    - Pas de canvas HTML5 lourd ni de SVG complexe pour le fond.
    - Utilisation de `background-image` répété pour une performance maximale et une maintenance nulle.

3.  **Architecture Modulaire :**
    - Les outils (`ConnectionTool`, `LayoutEngine`) sont des composants indépendants posés en absolu sur le canvas.
    - Facile d'ajouter un nouveau panneau (ex: "Propriétés") sans casser la logique de rendu.

---

## 💻 Exemple d'intégration

Le composant `DiagramCanvas` prend tout l'espace disponible de son parent.

```tsx
import DiagramCanvas from '@/components/diagram-editor/DiagramCanvas';

export default function ModelingPage() {
  return (
    <div style={{ height: 'calc(100vh - 64px)', width: '100%' }}>
      <DiagramCanvas />
    </div>
  );
}
```

---

## 🛠️ Évolutions possibles

- **Connexions Réelles :** Intégrer la logique de liens (SVG paths) vue dans le module `workflow-designer` pour relier les boîtes entre elles.
- **Zoom & Pan :** Ajouter la gestion de la transformation CSS (`transform: scale() translate()`) sur le conteneur des nœuds.
- **Sélection Multiple :** Permettre la sélection de plusieurs nœuds avec une "rubber band" (rectangle de sélection).
- **Snap to Grid :** Magnétisme automatique des nœuds sur la grille lors du relâchement.
