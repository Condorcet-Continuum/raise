# 🪝 Custom Hooks Library

Ce répertoire regroupe les hooks React personnalisés qui encapsulent la logique métier, la gestion d'état complexe et les interactions avec le backend Tauri (Rust). Ils permettent de garder les composants UI propres et focalisés sur le rendu.

---

## 📂 Catégorie : Intelligence Artificielle & Modélisation

Ces hooks pilotent le cœur "métier" de RAISE (MBAIE, Chat, Génération).

### `useAIChat` (`useAIChat.ts`)

Gère l'interaction conversationnelle avec l'assistant IA.

- **Rôle** : Interface entre l'UI de chat et le backend Rust (`invoke('ai_chat')`).
- **Store** : Connecté au `useAiStore` global pour la persistance de l'état (messages, loading).
- **Gestion des Artefacts** : Traite non seulement le texte, mais aussi les **Artefacts** visuels (Cartes JSON retournées par l'IA) qui sont injectés dans l'historique des messages.
- **Sécurité** : Gestion robuste des erreurs et état "Thinking".

### `useModelState` (`useModelState.ts`)

Façade simplifiée pour accéder au modèle Arcadia/SysML actif (via `ModelStore`).

- **Rôle** : Fournit un accès direct au projet courant et à l'élément sélectionné.
- **Fonctionnalité** : Transforme l'ID sélectionné (`selectedElementId`) en objet complet (`selectedElement`) pour faciliter l'affichage dans l'UI.
- **Usage** : Utilisé par les panneaux de propriétés et les vues de diagrammes.

### `useCodeGeneration` (`useCodeGeneration.ts`)

Pilote le processus de génération de code source (Rust, SQL, Python...) à partir du modèle.

- **Rôle** : Coordonne la demande vers le `codegenService` en utilisant le contexte du projet chargé.
- **Sécurité** : Vérifie qu'un projet est bien chargé avant de lancer la génération.
- **Utilitaire** : Inclut une méthode `copyToClipboard` pour copier le résultat généré.

---

## 📂 Catégorie : Moteur de Règles (GenRules)

### `useRulesEngine` (`useRulesEngine.ts`)

Gère l'interaction réactive avec le moteur de règles JSON-DB.

- **Rôle** : Synchronise un document "brouillon" (Draft) avec le backend pour recalculer les champs dérivés en temps réel.
- **Logique** :
  - **Debounce** : Temporise les appels au backend (défaut 500ms) pour éviter de surcharger le moteur pendant la frappe.
  - **Évaluation** : Appelle la commande Rust `jsondb_evaluate_draft`.
  - **Stabilité** : Utilise `useRef` pour comparer les états JSON et éviter les boucles infinies de rendu React.

---

## 📂 Catégorie : Utilitaires Système & Tauri

### `useFileSystem` (`useFileSystem.ts`)

Wrapper autour de l'API Fichiers de Tauri v2 (`@tauri-apps/plugin-fs`).

- **Rôle** : Simplifie la lecture et l'écriture de fichiers JSON typés.
- **Configuration** : Cible par défaut le répertoire `BaseDirectory.AppLocalData` pour sécuriser et isoler les données de l'application.

### `useTauriEvent` (`useTauriEvent.ts`)

Abonnement déclaratif aux événements globaux Tauri.

- **Rôle** : Attache un écouteur (`listen`) au montage du composant et le nettoie (`unlisten`) automatiquement au démontage.
- **Usage** : Indispensable pour écouter les logs asynchrones du backend ou les notifications push sans fuite de mémoire.

---

## 📦 Exemples d'Utilisation

### Exemple 1 : Chat IA

```typescript
import { useAIChat } from '@/hooks/useAIChat';

function ChatBox() {
  const { messages, sendMessage, isThinking } = useAIChat();

  return (
    <div>
      {messages.map((m) => (
        <div key={m.id}>{m.content}</div>
      ))}

      <input
        onKeyDown={(e) => e.key === 'Enter' && sendMessage(e.currentTarget.value)}
        disabled={isThinking}
      />
    </div>
  );
}
```

### Exemple 2 : Formulaire Réactif (Moteur de Règles)

```typescript
import { useRulesEngine } from '@/hooks/useRulesEngine';

function InvoiceForm() {
  // Le hook gère tout le cycle de vie : saisie -> debounce -> calcul -> mise à jour
  const { doc, handleChange, isCalculating } = useRulesEngine({
    space: 'demo',
    db: 'billing',
    collection: 'invoices',
    initialDoc: { qty: 1, price: 10 }, // Total calculé par le backend
  });

  return (
    <div>
      <input
        type="number"
        value={doc.qty as number}
        onChange={(e) => handleChange('qty', Number(e.target.value))}
      />
      {isCalculating && <span>Calcul...</span>}
      <div>Total (Calculé): {doc.total}</div>
    </div>
  );
}
```
