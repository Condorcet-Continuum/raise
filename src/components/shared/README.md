# Shared Components Library 🧱

Ce dossier contient la bibliothèque de composants UI génériques de GenAptitude.
Ces composants sont **atomiques**, **sans état métier** (stateless) et entièrement agnostiques du contexte d'utilisation (qu'ils soient dans le module Chat ou le module Modélisation).

Ils constituent la base du **Design System** de l'application.

---

## 📂 Inventaire des composants

| Fichier               | Rôle                                                                                                                  |
| --------------------- | --------------------------------------------------------------------------------------------------------------------- |
| **`ThemeToggle.tsx`** | Le bouton interrupteur (Soleil/Lune) qui gère le changement global de thème via l'attribut `data-theme` sur `<html>`. |
| **`Button.tsx`**      | Bouton standardisé avec variantes (`primary`, `secondary`, `ghost`). Gère les états hover/active.                     |
| **`Card.tsx`**        | Conteneur générique avec bordure, ombre et fond adapté au thème (`--bg-panel`).                                       |
| **`Modal.tsx`**       | Fenêtre de dialogue modale avec backdrop flouté et centrage automatique.                                              |
| **`Tabs.tsx`**        | Système d'onglets pour naviguer entre plusieurs vues sans rechargement.                                               |
| **`SplitPane.tsx`**   | Layout diviseur permettant de séparer l'écran en deux zones (Gauche/Droite) avec un ratio défini.                     |
| **`TreeView.tsx`**    | Composant récursif pour afficher des structures hiérarchiques (arbres de fichiers, modèles).                          |

---

## 🎨 Design & Thèmes

Tous les composants partagés sont construits pour réagir instantanément aux changements de variables CSS définies dans `src/styles/variables.css`.

### Règles d'implémentation :

1.  **Jamais de couleurs en dur :**

    - ❌ Pas de `background: #ffffff`
    - ✅ Utiliser `background: var(--bg-panel)` ou `var(--color-white)`

2.  **Typographie centralisée :**

    - Les polices, tailles et graisses proviennent des variables (`var(--font-size-sm)`, `var(--font-weight-bold)`).

3.  **Espacements cohérents :**
    - Les marges et paddings utilisent l'échelle `var(--spacing-...)`.

---

## 💻 Exemples d'utilisation

### 1. Bouton (`Button.tsx`)

```tsx
import { Button } from '@/components/shared/Button';

<Button variant="primary" onClick={doSomething}>
  Action Principale
</Button>

<Button variant="secondary">
  Annuler
</Button>
```

### 2\. Onglets (`Tabs.tsx`)

```tsx
import { Tabs } from '@/components/shared/Tabs';

const myTabs = [
  { id: 'tab1', label: 'Vue Code', content: <CodeEditor /> },
  { id: 'tab2', label: 'Vue Design', content: <Canvas /> },
];

<Tabs items={myTabs} initialId="tab1" />;
```

### 3\. Arbre (`TreeView.tsx`)

```tsx
import { TreeView } from '@/components/shared/TreeView';

const data = [
  {
    id: '1',
    label: 'Dossier A',
    children: [{ id: '2', label: 'Fichier B' }],
  },
];

<TreeView nodes={data} />;
```

---

## 🛠️ Maintenance

Lors de l'ajout d'un nouveau composant partagé :

1.  Vérifiez qu'il n'est pas lié à une logique métier spécifique (ex: pas d'appel API dans le composant).
2.  Assurez-vous qu'il utilise les variables CSS pour le rendu.
3.  Testez-le en **Mode Clair** ET en **Mode Sombre**.
