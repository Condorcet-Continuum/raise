# Module Diagram Editor ✏️

Ce module fournit un environnement de modélisation visuelle complet (canvas infini) pour RAISE.
Plus qu'un simple outil de dessin, il est **connecté en temps réel au "Cerveau" de l'IA** : les éléments définis textuellement dans le chat (ex: "Classe Client") apparaissent automatiquement ici sous forme graphique.

Il permet aux architectes systèmes de manipuler des diagrammes (SysML, Arcadia) via une interface fluide combinant Drag & Drop et outils vectoriels (SVG).

---

## 📂 Structure du dossier

| Fichier                 | Rôle                                                                                                                                                                          |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`DiagramCanvas.tsx`** | **Composant Maître**. Gère l'état des nœuds et des liens, le rendu de la couche SVG (flèches), la synchronisation avec le Store IA (`useAiStore`) et les interactions souris. |
| `ShapeLibrary.tsx`      | Barre latérale (Sidebar) contenant les éléments graphiques standards (Blocs, Acteurs, BDD...) prêts à être glissés-déposés.                                                   |
| `ConnectionTool.tsx`    | Barre d'outils flottante (Floating Toolbar) permettant de basculer entre les modes : Sélection, Lien (Connect), Texte et Suppression.                                         |
| `LayoutEngine.tsx`      | Panneau de contrôle pour déclencher les algorithmes de réorganisation automatique (Auto-layout).                                                                              |

---

## 🌟 Fonctionnalités Clés

### 1. Synchronisation IA (MBAIE) 🧠

L'éditeur écoute le flux de messages du chat (`useAiStore`).

- Dès qu'un artefact est généré par l'IA (ex: une nouvelle Class ou un Acteur), il est **automatiquement instancié** sur le diagramme.
- Les nœuds respectent le code couleur des couches Arcadia (DATA=Rouge, SA=Violet, etc.).

### 2. Création de Liens "Fil d'Ariane" 🔗

L'expérience utilisateur pour relier deux blocs a été optimisée :

1.  Activer l'outil **Lien** dans la barre flottante.
2.  Cliquer sur le nœud source.
3.  Une **ligne élastique pointillée** suit le curseur de la souris (feedback visuel immédiat).
4.  Cliquer sur le nœud cible pour valider la connexion.
5.  Appuyer sur `Echap` pour annuler.

### 3. Drag & Drop Natif

- Utilise l'API HTML5 Drag & Drop (`draggable`, `onDragStart`, `onDrop`) pour ajouter des formes manuelles depuis la `ShapeLibrary`.
- Transfert de données typées via `dataTransfer`.

### 4. Rendu Hybride Performant

- **Fond :** Grille CSS pure (`linear-gradient`) pour une performance maximale.
- **Nœuds :** Éléments HTML (`div`) pour faciliter le styling et le texte.
- **Liens :** Couche SVG superposée (`<svg>`) avec marqueurs de flèches (`<marker>`) pour des connecteurs vectoriels précis.

---

## 🎨 Design & Thèmes

L'éditeur s'intègre parfaitement au Design System de l'application :

- **Couleurs Arcadia :** Les nœuds utilisent automatiquement la couleur de leur couche (ex: `#ef4444` pour DATA).
- **Mode Sombre/Clair :** Toutes les couleurs (fond, grille, bordures) utilisent des variables CSS globales (`var(--bg-app)`, `var(--text-main)`).
- **Indicateurs Visuels :**
  - **Survol :** Effet de scale léger.
  - **Sélection :** Bordure accentuée (`var(--color-primary)`).
  - **Liaison :** Le curseur change (`crosshair`) et la source est mise en surbrillance.

---

## 💻 Exemple d'intégration

Le composant `DiagramCanvas` est autonome et responsive.

```tsx
import DiagramCanvas from '@/components/diagram-editor/DiagramCanvas';

export default function ModelingPage() {
  return (
    <div style={{ height: '100%', width: '100%', position: 'relative' }}>
      <DiagramCanvas />
    </div>
  );
}
```

---

## 🛠️ Évolutions futures (Roadmap)

- **Édition de texte :** Double-cliquer sur un nœud pour renommer le label.
- **Algorithmes de Layout :** Implémenter la logique réelle dans `LayoutEngine` (ex: via `elkjs` ou `dagre`) pour organiser proprement les nœuds importés de l'IA.
- **Sélection Multiple :** Rectangle de sélection ("Rubber band") pour déplacer des groupes.
- **Persistance :** Sauvegarder la position des nœuds dans la base de données locale ou le JSON projet.

```

```
