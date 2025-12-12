# Module Code Generator (Usine Logicielle) ⚙️

Ce module est responsable de la transformation des modèles d'architecture (Arcadia/SysML) en code source exécutable (Rust, Python, C++).
Il agit comme une interface de contrôle pour le moteur de génération, permettant de sélectionner la cible, de visualiser le résultat et de copier le code généré.

---

## 📂 Structure du dossier

| Fichier                 | Rôle                                                                                                                                                |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`CodeGenerator.tsx`** | **Composant Principal**. Interface utilisateur complète incluant la barre d'outils, le sélecteur de langage et la zone de prévisualisation du code. |

---

## 🎨 Design & Thèmes

Le générateur de code adopte une esthétique "IDE" (Environnement de Développement Intégré) tout en respectant le système de thèmes global.

### Adaptation chromatique :

- **Zone d'Édition :** Utilise `var(--bg-app)` (Gris très clair en Light / Gris très foncé en Dark) pour simuler une zone de texte neutre, distincte du panneau principal.
- **Typographie du Code :** `var(--font-family-mono)` est utilisée pour garantir un alignement parfait du code généré.
- **Barre d'outils :** `var(--color-gray-50)` pour se détacher légèrement du fond du panel (`var(--bg-panel)`).
- **Bouton Générer :** `var(--color-primary)` pour l'action principale.
- **Feedback Utilisateur :**
  - Succès (Copie) : `var(--color-success)`.
  - Erreur : `var(--color-error)`.

---

## 💻 Fonctionnalités

1.  **Sélection de la Cible :**

    - **Rust (System) :** Pour les composants haute performance.
    - **Python (Scripting) :** Pour l'orchestration ou l'analyse de données.
    - **C++ (Embedded) :** Pour les cibles embarquées temps-réel.

2.  **Interaction avec le Store :**

    - Le composant s'abonne au `model-store` pour récupérer le projet actif (`currentProject`).
    - Si aucun projet n'est chargé, il affiche un message d'erreur clair à l'utilisateur.

3.  **Service de Génération :**
    - Appelle `codegenService.generateCode()` (simulation asynchrone) pour transformer le modèle JSON en chaîne de caractères.
    - Gère les états de chargement (`loading`) et d'erreur (`error`).

---

## 💻 Exemple d'intégration

Ce composant est autonome et conçu pour être affiché dans une page dédiée ou un onglet principal.

```tsx
import CodeGenerator from '@/components/codegen/CodeGenerator';

export default function CodegenPage() {
  return (
    <div style={{ height: '100%', padding: '20px' }}>
      <CodeGenerator />
    </div>
  );
}
```

## 🛠️ Évolutions possibles

- **Syntax Highlighting :** Intégrer le composant `SyntaxHighlighter` du module `code-editor` pour colorer le code généré (actuellement en texte brut).
- **Diff View :** Afficher les différences entre la version précédente et la nouvelle version générée.
- **Téléchargement :** Ajouter un bouton pour télécharger le résultat sous forme de fichier `.rs`, `.py` ou `.cpp`.

<!-- end list -->
