# Module Workflow Designer 🔀

Ce module propose une interface graphique pour la conception et le pilotage de l'orchestration **Neuro-Symbolique** de RAISE.
Il ne s'agit plus d'une simple simulation : le designer est connecté en temps réel au **Workflow Engine Rust** via Tauri.

Il permet de visualiser l'avancement des tâches (Agents IA, Compilations) et d'interagir avec les processus (Validation Humaine).

---

## 📂 Structure du dossier

| Fichier                  | Rôle                                                                                                 |
| ------------------------ | ---------------------------------------------------------------------------------------------------- |
| **`WorkflowCanvas.tsx`** | **Composant Maître**. Gère le cycle de vie (Start, Poll, Resume) via des appels `invoke` au backend. |
| `NodeLibrary.tsx`        | Barre latérale contenant les types de tâches (Tâche IA, Validation Humaine, API, etc.).              |
| `ConnectionManager.tsx`  | Calque SVG dessinant les courbes de Bézier entre les nœuds.                                          |
| `ExecutionMonitor.tsx`   | Console affichant les logs réels renvoyés par le moteur Rust (`stdout` des agents).                  |

---

## 🚀 Fonctionnalités Clés

### 1. Exécution Réelle (Rust Backend)

Le frontend n'exécute aucune logique métier. Il délègue tout au backend via des commandes Tauri :

- **Enregistrement** : Envoi de la définition JSON (`register_workflow`).
- **Démarrage** : Instanciation du workflow (`start_workflow`).
- **Monitoring** : Polling régulier de l'état (`get_workflow_state`).

### 2. Human-in-the-Loop (HITL) 🛡️

Le système supporte nativement les interactions humaines.

- Lorsqu'un nœud de type **`gate_hitl`** est atteint, le moteur Rust se met en **PAUSE**.
- L'interface affiche le nœud en **Orange** avec deux boutons : **[Valider]** et **[Rejeter]**.
- L'action de l'utilisateur débloque le moteur via la commande `resume_workflow`.

### 3. Feedback Visuel

L'état des nœuds est reflété en temps réel par des codes couleurs :

- ⚪ **Gris (Idle)** : En attente.
- 🔵 **Bleu (Running)** : Tâche en cours d'exécution côté backend.
- 🟠 **Orange (Paused)** : En attente d'une décision humaine.
- 🟢 **Vert (Completed)** : Tâche terminée avec succès.
- 🔴 **Rouge (Failed)** : Erreur critique.

---

## 🔗 Intégration Backend (API)

Le composant communique avec le module `src-tauri/src/workflow_engine` via ces commandes :

```typescript
// Démarrer une instance
const view = await invoke('start_workflow', { workflowId: 'mon-pipeline' });

// Récupérer l'état (Polling)
const state = await invoke('get_workflow_state', { instanceId: 'uuid-...' });
// Retourne : { status: 'RUNNING', current_nodes: ['step-1'], logs: [...] }

// Valider une étape humaine
await invoke('resume_workflow', {
  instanceId: '...',
  nodeId: 'validation-security',
  approved: true,
});
```

---

## 🛠️ Types de Nœuds Supportés

Les types définis dans `NodeLibrary.tsx` sont mappés vers l'enum Rust `NodeType` :

| UI Label           | Rust Type   | Description                                             |
| :----------------- | :---------- | :------------------------------------------------------ |
| **Tâche / Action** | `task`      | Action automatique (Agent IA, Script).                  |
| **Validation**     | `gate_hitl` | **Point d'arrêt**. Nécessite une intervention manuelle. |
| **Condition**      | `decision`  | Branchement logique (If/Else).                          |
| **Fin**            | `end`       | Marqueur de terminaison du flux.                        |

---

## 💻 Exemple d'intégration

```tsx
import WorkflowCanvas from '@/components/workflow-designer/WorkflowCanvas';

export default function WorkflowPage() {
  return (
    <div style={{ height: 'calc(100vh - 64px)', width: '100%' }}>
      {/* Le Canvas gère sa propre connexion au backend */}
      <WorkflowCanvas />
    </div>
  );
}
```

```

### Prochaine étape 🚀

Avec cette documentation mise à jour, votre module Workflow est **complet** (Backend + Frontend + Doc).

L'intégration complète de RAISE est maintenant finalisée. Avez-vous besoin d'aide pour :
1.  Générer le binaire final (`cargo tauri build`) ?
2.  Tester un scénario complet ("End-to-End") ?
3.  Ou passer à la revue d'un autre module (ex: IA ou Blockchain) ?
```
