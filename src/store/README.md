# Global State Management 📦

Ce dossier contient la logique de gestion d'état global de GenAptitude.
Nous utilisons **Zustand** pour sa simplicité, sa performance (pas de re-rendus inutiles) et son API basée sur les Hooks.

---

## 📂 Inventaire des Stores

| Fichier                 | Rôle                                                                                                                                                                  |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`model-store.ts`**    | **Store Critique**. Gère le modèle d'architecture (Arcadia/SysML) chargé en mémoire. Il contient la logique d'indexation pour accéder rapidement aux éléments par ID. |
| **`ai-store.ts`**       | Gère l'historique de conversation avec l'assistant IA, ainsi que les états de chargement (`isThinking`) et d'erreur.                                                  |
| **`ui-store.ts`**       | Gère l'état purement visuel : Thème (Light/Dark), ouverture de la Sidebar, disposition des panneaux (Split/Single).                                                   |
| **`settings-store.ts`** | Contient la configuration de l'application (Langue, choix du backend IA, paramètres de base de données).                                                              |
| **`project-store.ts`**  | Gère la liste des projets récents ou disponibles (méta-données : chemin, nom, domaine).                                                                               |

---

## 🏗️ Architecture & Patterns

### 1. Indexation à plat (Flat Indexing)

Dans `model-store.ts`, nous appliquons un pattern d'optimisation important pour les gros modèles.
Au lieu de parcourir l'arbre récursivement à chaque fois qu'on cherche un élément, nous maintenons un index plat `elementsById`.

- **Avantage :** Accès en **O(1)** pour trouver n'importe quel élément (ex: pour l'inspecteur de propriétés).
- **Implémentation :** L'action `setProject` parcourt le modèle une seule fois au chargement pour remplir cet index.

### 2. Séparation UI / Data

- **`ui-store`** ne contient que ce qui est éphémère à l'interface (ex: est-ce que le menu est ouvert ?).
- **`model-store`** contient la donnée métier persistante.

### 3. Actions atomiques

Les stores exposent des actions précises (`addMessage`, `updateElement`, `toggleSidebar`) plutôt que de laisser les composants modifier l'état directement. Cela centralise la logique de mutation.

---

## 💻 Exemples d'utilisation

### Accéder à une donnée

```tsx
import { useModelStore } from '@/store/model-store';

export function ProjectTitle() {
  // Sélectionner uniquement ce dont on a besoin évite les re-rendus inutiles
  const project = useModelStore((state) => state.project);

  if (!project) return null;
  return <h1>{project.name}</h1>;
}
```

### Déclencher une action

```tsx
import { useUiStore } from '@/store/ui-store';

export function ToggleButton() {
  const toggleSidebar = useUiStore((state) => state.toggleSidebar);

  return <button onClick={toggleSidebar}>Menu</button>;
}
```

### Mise à jour d'un élément (Pattern Optimiste)

```tsx
import { useModelStore } from '@/store/model-store';

const updateElement = useModelStore((state) => state.updateElement);

// Met à jour le nom immédiatement dans le store (et donc l'UI)
updateElement('element-uuid-123', { name: 'Nouveau Nom' });
```

---

## 🛠️ Maintenance

- **Persistance :** Actuellement, les stores sont en mémoire vive (RAM). Pour persister des données (ex: Préférences utilisateur) entre les rechargements, il faudra ajouter le middleware `persist` de Zustand dans `settings-store.ts`.
- **Typage :** Toujours définir une interface `State` et l'utiliser dans `create<State>(...)` pour garantir l'autocomplétion TypeScript.

```

```
