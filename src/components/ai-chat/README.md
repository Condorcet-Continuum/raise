# Module AI Chat 🤖

Ce module implémente l'interface conversationnelle de l'assistant **GenAptitude**.
Il a été entièrement refactorisé pour supporter le **Thème Dynamique (Light/Dark Mode)** et utilise une architecture de composants atomiques.

---

## 📂 Structure du dossier

| Fichier                 | Rôle                                                                                                                       |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| **`ChatInterface.tsx`** | **Composant Maître**. Il assemble tous les sous-composants et gère la logique d'affichage globale via le hook `useAIChat`. |
| `MessageBubble.tsx`     | Affiche un message unique. Gère la distinction visuelle entre l'utilisateur (Bleu/Primary) et l'IA (Gris/Neutre).          |
| `InputBar.tsx`          | Zone de saisie (Textarea) avec gestion de la soumission (Enter) et bouton d'envoi.                                         |
| `SuggestionPanel.tsx`   | Affiche des "Chips" cliquables (Prompts suggérés) pour guider l'utilisateur.                                               |
| `IntentClassifier.tsx`  | Composant d'analyse visuelle qui détecte le contexte de la question (ex: "DevOps", "Modélisation").                        |
| `ContextDisplay.tsx`    | Affiche les métadonnées discrètes de la session (compteur de messages).                                                    |

---

## 🎨 Système de Design & Thèmes

Ce module n'utilise **aucune couleur hexadécimale en dur** (`#ffffff`, `#000000`).
Il repose exclusivement sur les variables CSS définies dans `src/styles/variables.css` pour garantir la compatibilité automatique avec le mode sombre.

### Mapping des couleurs clés :

- **Conteneur Principal :** `var(--bg-panel)` (Blanc en Light / Gris foncé en Dark).
- **Bulle Utilisateur :** `var(--color-primary)` (Indigo). Le texte est forcé en blanc pour le contraste.
- **Bulle Assistant :** `var(--color-gray-100)`. S'inverse automatiquement (Gris clair en Light / Gris foncé en Dark).
- **Texte :** `var(--text-main)` et `var(--text-muted)`.

---

## 💻 Exemple d'intégration

Le composant `ChatInterface` est conçu pour occuper 100% de la hauteur de son conteneur parent.

```tsx
import { ChatInterface } from '@/components/ai-chat/ChatInterface';

export default function AiPage() {
  return (
    <div style={{ height: 'calc(100vh - 80px)', padding: '20px' }}>
      <ChatInterface />
    </div>
  );
}
```

## 🔗 Dépendances

Ce module dépend des éléments suivants :

1.  **Hooks :** `useAIChat` (Logique métier, envoi de messages, états de chargement).
2.  **Types :** `ChatMessage` (Interface définie dans `@/store/ai-store`).
3.  **Styles :** `src/styles/globals.css` (Doit être importé à la racine de l'app).

## 🛠️ Extensions futures possibles

- Ajouter le support du **Markdown** dans `MessageBubble` pour le rendu de code.
- Implémenter le **Streaming** de réponse (effet machine à écrire).
- Ajouter un bouton pour **copier le contenu** d'une réponse.
