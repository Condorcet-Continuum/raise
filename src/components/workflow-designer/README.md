# Module Workflow Designer 🔀

Ce module propose une interface graphique de type "Node-Based" (Nœuds et Liens) pour l'orchestration de tâches complexes (Pipelines CI/CD, ETL de données, Automatisations).
Il permet de glisser-déposer des briques fonctionnelles, de visualiser leurs connexions et de simuler leur exécution.

---

## 📂 Structure du dossier

| Fichier                  | Rôle                                                                                                   |
| ------------------------ | ------------------------------------------------------------------------------------------------------ |
| **`WorkflowCanvas.tsx`** | **Composant Maître**. Gère la zone de travail, l'état des nœuds et coordonne le Drag & Drop.           |
| `NodeLibrary.tsx`        | Barre latérale (Sidebar) contenant les types de tâches disponibles (Trigger, Action, Condition, etc.). |
| `ConnectionManager.tsx`  | Calque SVG superposé au canvas pour dessiner les courbes de Bézier reliant les nœuds.                  |
| `ExecutionMonitor.tsx`   | Console rétractable en bas d'écran affichant les logs d'exécution en temps réel.                       |

---

## 🎨 Design & Thèmes

L'interface est conçue pour être claire et lisible, même avec des graphes complexes.

### Adaptation chromatique :

- **Le Canvas :** Utilise `var(--bg-app)` avec un motif radial subtil (`background-image`) pour guider l'alignement sans surcharger la vue.
- **Les Nœuds :**
  - Chaque type de nœud possède un code couleur sémantique (via `border-left` et pastilles) :
    - ⚡ **Déclencheur :** Warning (Jaune/Orange)
    - ⚙️ **Action :** Primary (Indigo)
    - 🛑 **Terminaison :** Error (Rouge)
  - Les fonds s'adaptent au thème (`var(--bg-panel)`).
- **Les Connexions :** Dessinées en SVG avec `stroke="var(--color-gray-400)"`, ce qui assure une visibilité correcte sur fond clair comme sur fond sombre.
- **Console :** Ressemble à un terminal avec une police Monospace et des couleurs de logs contextuelles (Vert pour succès, Rouge pour erreur).

---

## 💻 Fonctionnalités

1.  **Architecture en Couches (Layers) :**

    - **Couche 0 (Fond) :** Grille CSS.
    - **Couche 1 (SVG) :** `ConnectionManager` qui trace les lignes. `pointer-events: none` permet de cliquer "au travers" pour sélectionner le fond.
    - **Couche 2 (DOM) :** Les `div` des nœuds positionnés en absolu.

2.  **Drag & Drop :**

    - Ajout de nouveaux nœuds depuis la bibliothèque vers le canvas.
    - Déplacement des nœuds existants (mise à jour fluide des coordonnées).

3.  **Simulation d'Exécution :**
    - Le composant `ExecutionMonitor` simule un processus asynchrone (Build, Test, Deploy) et affiche les logs ligne par ligne pour valider la logique du workflow.

---

## 💻 Exemple d'intégration

Le designer est conçu pour occuper l'intégralité de l'écran ou d'un onglet.

```tsx
import WorkflowCanvas from '@/components/workflow-designer/WorkflowCanvas';

export default function PipelinePage() {
  return (
    <div style={{ height: 'calc(100vh - 64px)', width: '100%' }}>
      <WorkflowCanvas />
    </div>
  );
}
```

---

## 🛠️ Évolutions possibles

- **Édition des Liens :** Permettre de cliquer sur deux nœuds pour créer une connexion dynamiquement (actuellement les liens sont statiques pour la démo).
- **Zoom & Pan :** Comme pour l'éditeur de diagrammes, ajouter la navigation spatiale sur le canvas infini.
- **Inspecteur de Nœud :** Cliquer sur un nœud pour ouvrir un panneau latéral et configurer ses paramètres (ex: URL du webhook, script bash à exécuter).
- **Export YAML/JSON :** Sérialiser le graphe pour le sauvegarder ou le transformer en fichier GitHub Actions / GitLab CI.
