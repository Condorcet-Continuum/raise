# Module Code Editor 💻

Ce module fournit un environnement d'édition de code léger, performant et entièrement intégré au design system de RAISE.
Il est conçu pour l'édition de configurations (JSON), de scripts (JS/TS) ou la visualisation de code généré, sans la lourdeur d'une librairie externe comme Monaco Editor.

---

## 📂 Structure du dossier

| Fichier                 | Rôle                                                                                                                       |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| **`CodeEditor.tsx`**    | **Composant Maître**. Orchestre la zone de saisie (`textarea`), la numérotation des lignes et l'appel aux sous-composants. |
| `SyntaxHighlighter.tsx` | Composant de rendu (lecture seule) qui colore la syntaxe (JSON/JS) en utilisant les variables de thème.                    |
| `CodeCompletion.tsx`    | Popup flottante (Popover) qui affiche les suggestions d'autocomplétion contextuelles.                                      |
| `LivePreview.tsx`       | Panneau latéral optionnel pour visualiser le résultat du code en temps réel (ex: rendu JSON formaté).                      |

---

## 🎨 Système de Design & Thèmes

L'éditeur respecte scrupuleusement les thèmes (Clair/Sombre) grâce aux variables CSS.

### Adaptation chromatique :

- **Fond de l'éditeur :** `var(--bg-panel)`.
- **Fond de la gouttière (numéros de ligne) :** `var(--bg-app)` pour créer une séparation visuelle subtile.
- **Texte :** `var(--font-family-mono)` pour l'alignement, couleur `var(--text-main)`.
- **Coloration Syntaxique :**
  - **Clés / Mots-clés :** `var(--color-primary)` (Indigo).
  - **Chaînes de caractères :** `var(--color-success)` (Vert).
  - **Booléens / Nombres :** `var(--color-warning)` (Orange).
  - **Ponctuation :** `var(--color-accent)` (Violet).

Cela garantit que le code reste lisible même si l'utilisateur change de thème à la volée.

---

## 💻 Exemple d'intégration

Voici comment intégrer l'éditeur complet avec gestion d'état :

```tsx
import { useState } from 'react';
import { CodeEditor } from '@/components/code-editor/CodeEditor';
import { LivePreview } from '@/components/code-editor/LivePreview';

export function ConfigPage() {
  const [code, setCode] = useState('{\n  "projet": "RAISE",\n  "version": 1.0\n}');

  return (
    <div style={{ display: 'flex', height: '500px', gap: '20px' }}>
      {/* Zone d'édition (60%) */}
      <div style={{ flex: 1 }}>
        <CodeEditor
          value={code}
          onChange={setCode}
          language="json"
          placeholder="Saisissez votre configuration..."
        />
      </div>

      {/* Aperçu en temps réel (40%) */}
      <div style={{ flex: 0.6 }}>
        <LivePreview content={code} format="json" />
      </div>
    </div>
  );
}
```
