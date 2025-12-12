# Services Layer (Bridge) 🔌

Ce répertoire contient la logique métier de l'application et les points d'entrée vers le backend Rust via **Tauri IPC**.
L'objectif de cette couche est de découpler l'interface utilisateur (UI) de la logique de données et des appels système.

Les composants UI ne doivent jamais appeler `invoke()` directement, mais passer par ces services.

---

## 📂 Inventaire des Services

| Fichier                   | Rôle                                                                                                                                         |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| **`json-db/`**            | **Sous-module**. SDK complet pour la base de données NoSQL/SQL (Collections, Requêtes, Transactions). Voir le [README](./json-db/README.md). |
| **`ai-service.ts`**       | Gestion des interactions avec le LLM (Chat), récupération du statut du système IA et tests NLP.                                              |
| **`model-service.ts`**    | Chargement et sauvegarde des modèles d'architecture (Arcadia/SysML). Connecté au `settings-store` pour cibler la bonne DB.                   |
| **`codegenService.ts`**   | Pilote l'usine logicielle : transforme le modèle JSON en code source (Rust/Python/C++) via le moteur de templates Rust.                      |
| **`geneticsService.ts`**  | Interface pour lancer les algorithmes d'optimisation génétique (calcul lourd côté Rust).                                                     |
| **`cognitiveService.ts`** | Service d'analyse de cohérence sémantique (vérification des règles métiers).                                                                 |
| **`file-service.ts`**     | Gestion des fichiers natifs (Ouvrir/Enregistrer) via les plugins officiels `@tauri-apps/plugin-dialog` et `fs`.                              |
| **`tauri-commands.ts`**   | Registre de constantes contenant les noms exacts des commandes Rust (ex: `'jsondb_insert_document'`). Évite les "Magic Strings".             |

---

## 🏗️ Architecture & Patterns

### 1. Singleton Pattern

Chaque service est instancié une seule fois et exporté directement.
Cela permet de conserver une instance unique dans toute l'application.

```typescript
// Définition
class MyService { ... }
// Export
export const myService = new MyService();
```

### 2\. Configuration Dynamique

Les services ne stockent pas d'état persistant (sauf cache temporaire). Ils récupèrent leur configuration (ex: quelle Base de Données utiliser ?) directement depuis le **Store Global** au moment de l'appel.

```typescript
// Exemple dans model-service.ts
async loadProject() {
  // Récupération dynamique de la config
  const { jsonDbSpace } = useSettingsStore.getState();
  // Appel Backend
  return await invoke('load_project', { space: jsonDbSpace });
}
```

### 3\. Gestion des Erreurs

Les services interceptent les erreurs techniques de Tauri (`invoke` rejection) et les normalisent ou les loggent avant de les propager à l'UI.

---

## 🔗 Correspondance Tauri (Rust)

Ces services sont les miroirs des commandes définies dans `src-tauri/src/commands/*.rs`.

| Service TS                | Commande Rust              | Description                                  |
| ------------------------- | -------------------------- | -------------------------------------------- |
| `aiService.chat`          | `ai_chat`                  | Envoie un prompt au LLM local/distant.       |
| `modelService.load`       | `load_project_model`       | Désérialise un projet complexe depuis la DB. |
| `codegenService.generate` | `generate_source_code`     | Utilise le moteur Tera pour générer du code. |
| `geneticsService.run`     | `run_genetic_optimization` | Lance une simulation longue (threadé).       |

---

## 🛠️ Maintenance

Lors de l'ajout d'une nouvelle fonctionnalité backend :

1.  Ajoutez le nom de la commande dans `tauri-commands.ts`.
2.  Créez une méthode typée dans le service correspondant (ou créez-en un nouveau).
3.  Utilisez ce service dans vos Hooks ou Composants.
