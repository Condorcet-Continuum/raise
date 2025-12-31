# Module AI Chat 🤖

Ce module implémente l'interface conversationnelle centrale de l'assistant **RAISE**.
Il ne s'agit pas d'un simple chat textuel : c'est une console **MBAIE (Model-Based AI Engineering)** capable de générer des artefacts structurés, de les visualiser et de déclencher des actions d'ingénierie (génération de code) via des interactions UI.

---

## 📂 Structure des composants

| Fichier                 | Rôle et Responsabilités                                                                                                                                            |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **`ChatInterface.tsx`** | **Composant Maître**. Orchestre le flux de messages via `useAIChat`. Gère l'injection des prompts techniques (ex: génération de code Rust/SQL) suite aux clics UI. |
| `MessageBubble.tsx`     | Affiche un message unique. Si le message contient des `artifacts` (données structurées), il instancie une ou plusieurs `ArtifactCard`.                             |
| **`ArtifactCard.tsx`**  | **Nouveau**. Affiche une "carte" interactive pour un élément généré (ex: Classe, Acteur). Gère le menu contextuel pour la génération de code (Rust, SQL, Python).  |
| `InputBar.tsx`          | Zone de saisie utilisateur avec gestion des états (disabled pendant la réflexion de l'IA).                                                                         |
| `SuggestionPanel.tsx`   | Affiche des puces de suggestions (Prompts rapides) pour guider l'utilisateur.                                                                                      |
| `IntentClassifier.tsx`  | Analyseur visuel léger qui détecte l'intention du dernier message (ex: "Modélisation", "DevOps") par mots-clés.                                                    |
| `ContextDisplay.tsx`    | Affiche discrètement les métadonnées de la session (nombre de messages).                                                                                           |

---

## ⚙️ Workflow MBAIE (Model-Based AI Engineering)

Ce module implémente une boucle de feedback ingénierie complète :

1.  **Intention Utilisateur** : L'utilisateur demande "Défini la classe Client".
2.  **Structuration** : Le Backend répond avec du texte ET un artefact JSON structuré (`CreatedArtifact`).
3.  **Visualisation** : `MessageBubble` détecte l'artefact et rend une `ArtifactCard` colorée selon la couche Arcadia (DATA, SA, LA...).
4.  **Action** : L'utilisateur clique sur **"Générer Rust"** dans la carte.
5.  **Prompting Automatique** : `ChatInterface` intercepte l'événement et envoie un prompt contextuel expert à l'IA (_"Agis en tant qu'expert Software, génère le code pour l'élément X..."_).

---

## 🎨 Système de Design & Couleurs Arcadia

Le module respecte le thème dynamique (Light/Dark) via les variables CSS globales, mais introduit également une **grammaire visuelle spécifique à l'ingénierie système (Arcadia)** dans les `ArtifactCard`.

### Couleurs des Couches (Layers)

Défini dans `ArtifactCard.tsx` :

| Couche         | Code Couleur       | Usage                                  |
| -------------- | ------------------ | -------------------------------------- |
| **OA**         | `#eab308` (Jaune)  | Analyse Opérationnelle                 |
| **SA**         | `#a855f7` (Violet) | Analyse Système                        |
| **LA**         | `#3b82f6` (Bleu)   | Architecture Logique                   |
| **PA**         | `#22c55e` (Vert)   | Architecture Physique                  |
| **EPBS**       | `#f97316` (Orange) | Breakdown Structure (Produit)          |
| **DATA**       | `#ef4444` (Rouge)  | Modélisation de données (Classes, ERD) |
| **TRANSVERSE** | `#64748b` (Gris)   | Éléments génériques ou inconnus        |

### Mapping Thème (UI Générale)

- **Conteneur :** `var(--bg-panel)`
- **Bulle Utilisateur :** `var(--color-primary)` (Indigo).
- **Bulle IA :** `var(--color-gray-100)`.
- **Texte :** `var(--text-main)` et `var(--text-muted)`.

---

## 💻 Exemple d'intégration

```tsx
import { ChatInterface } from '@/components/ai-chat/ChatInterface';

export default function AiPage() {
  return (
    // Le conteneur doit avoir une hauteur définie pour le scroll interne
    <div style={{ height: 'calc(100vh - 80px)', padding: '20px' }}>
      <ChatInterface />
    </div>
  );
}
```

````

## 🔗 Dépendances

1. **Store & Types :** `@/types/ai.types` (Interfaces `ChatMessage`, `CreatedArtifact`).
2. **Hooks :** `@/hooks/useAIChat` (Logique de communication avec le backend Rust).
3. **Styles :** Variables CSS globales.

```

```
````
