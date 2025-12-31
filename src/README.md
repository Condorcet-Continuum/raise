# ⚛️ Frontend Architecture (RAISE)

Ce dossier contient le code source de l'interface utilisateur de **RAISE**.
L'application est une Single Page Application (SPA) robuste construite avec **React 18**, **TypeScript** et **Vite**, conçue pour piloter un backend Rust performant via l'API **Tauri v2**.

## 🛠️ Stack Technique

| Technologie     | Usage            | Justification                                                                       |
| :-------------- | :--------------- | :---------------------------------------------------------------------------------- |
| **React 18**    | UI Framework     | Composants fonctionnels et Hooks pour une UI réactive.                              |
| **TypeScript**  | Langage          | Typage strict pour garantir la cohérence avec les structures Rust (Structs).        |
| **Vite**        | Build Tool       | Démarrage instantané et HMR (Hot Module Replacement) ultra-rapide.                  |
| **Zustand**     | State Management | Gestion d'état global minimaliste (remplace Redux) pour éviter le "Prop Drilling".  |
| **CSS Modules** | Styling          | Styles scopés localement avec support natif des variables CSS (Thème Sombre/Clair). |
| **Tauri API**   | Bridge Backend   | Communication asynchrone (`invoke`, `listen`) avec le noyau Rust.                   |

---

## 📂 Organisation du Code

L'architecture suit une séparation stricte des responsabilités (MVC-like) adaptée au Frontend :

```text
src/
├── assets/             # Images, icônes et polices statiques
├── components/         # Bibliothèque de composants UI (Voir README interne)
│   ├── layout/         # Structure (Header, Sidebar)
│   ├── shared/         # Composants atomiques réutilisables
│   ├── rules_engine/   # Démo Moteur de Règles
│   └── ...             # Modules métier (ModelViewer, Blockchain, etc.)
│
├── hooks/              # Custom Hooks (Logique réutilisable)
│   ├── useRulesEngine  # Hook réactif pour le moteur GenRules
│   ├── useAIChat       # Hook pour les LLM
│   └── ...
│
├── services/           # Couche de service (API Rust & Logique pure)
│   ├── json-db/        # Wrappers pour la base de données JSON
│   ├── model-service   # Gestion du modèle Arcadia
│   └── ...
│
├── store/              # Gestion d'état global (Zustand)
│   ├── model-store.ts  # État du projet courant
│   ├── settings-store.ts # Configuration app (IA, DB path)
│   └── ...
│
├── styles/             # Fichiers CSS globaux et variables de thème
├── types/              # Définitions TypeScript partagées (Interfaces, Enums)
├── utils/              # Fonctions utilitaires (Parsers, Formatters)
│
├── App.tsx             # Routeur principal et Orchestration
└── main.tsx            # Point d'entrée (Mount React DOM)
```

---

## 🧩 Catalogue des Composants

Les composants sont regroupés par domaine fonctionnel. Voici les modules clés de l'application :

| Module (Dossier)    | Composants Clés                                         | Description & Responsabilité                                                                                                                          |
| :------------------ | :------------------------------------------------------ | :---------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`layout/`**       | `MainLayout`, `Sidebar`, `Header`                       | **Squelette de l'application**. Gère la navigation, le titre dynamique et le basculement du thème.                                                    |
| **`dashboard/`**    | `DashboardView`                                         | **Page d'accueil**. Affiche les KPIs du projet, l'état de connexion au backend Rust et les raccourcis.                                                |
| **`rules_engine/`** | `RulesEngineDashboard`, `InvoiceDemo`, `ModelRulesDemo` | **Moteur Réactif**. Interface de démonstration du calcul en temps réel (GenRules). Affiche les champs calculés et les validations sans sauvegarde DB. |
| **`settings/`**     | `SettingsPage`                                          | **Configuration**. Gestion des préférences (Backend IA, Chemins DB) et test de connexion au modèle.                                                   |
| **`blockchain/`**   | `BlockchainView`, `BlockchainToast`                     | **Ledger**. Visualisation de l'ancrage des preuves sur Hyperledger Fabric.                                                                            |
| **`model-viewer/`** | `CapellaViewer`                                         | **Explorateur**. Vue en arbre et détails des éléments du modèle Arcadia (Logical/Physical Architecture).                                              |
| **`ai-chat/`**      | `ChatInterface`, `InputBar`                             | **Assistant**. Interface conversationnelle connectée aux LLM locaux ou distants.                                                                      |
| **`shared/`**       | `Button`, `Card`, `Modal`                               | **Atomique**. Composants visuels purs, sans logique métier, réutilisables partout.                                                                    |
| **`(Racine)`**      | `JsonDbTester`, `CognitiveTester`                       | **Diagnostic**. Outils "Bas niveau" pour administrer la DB ou tester les modules WASM directement.                                                    |

---

## 🔌 Services & Hooks (Lien Frontend-Backend)

C'est ici que s'opère la magie. Le Frontend n'appelle jamais Rust directement depuis les composants (sauf rares exceptions). Il passe par des **Hooks** ou des **Services** typés.

### 1\. Custom Hooks (`src/hooks/`)

Encapsulent la logique d'état complexe et le cycle de vie React.

| Hook                    | Rôle                                                                                    | Commande Tauri associée |
| :---------------------- | :-------------------------------------------------------------------------------------- | :---------------------- |
| **`useRulesEngine`**    | Gère le "Debounce" de saisie et la mise à jour des champs calculés (Factures, Règles).  | `jsondb_evaluate_draft` |
| **`useAIChat`**         | Gère l'historique de chat, l'état "Thinking" et le choix du backend IA (Mock vs Local). | `ai_chat`               |
| **`useCodeGeneration`** | Pilote la génération de code source à partir du modèle chargé.                          | `generate_source_code`  |
| **`useModelState`**     | Façade pour accéder et manipuler le `ProjectModel` courant (Sélection, Updates).        | _N/A (Zustand)_         |
| **`useFileSystem`**     | Utilitaire pour lire/écrire des fichiers JSON locaux (via Tauri FS Plugin).             | _Tauri Plugin FS_       |

### 2\. Services (`src/services/`)

Fonctions asynchrones pures qui effectuent les appels `invoke` vers Rust.

| Service                 | Méthodes Clés                                          | Description                                                       |
| :---------------------- | :----------------------------------------------------- | :---------------------------------------------------------------- |
| **`modelService`**      | `loadProjectModel(space, db)`                          | Charge un modèle Arcadia complet en mémoire depuis JSON-DB.       |
| **`collectionService`** | `createCollection`, `insertDocument`, `queryDocuments` | CRUD complet sur la base de données (utilisé par `JsonDbTester`). |
| **`codegenService`**    | `generateCode(lang, model)`                            | Transforme le modèle en code source (Rust, Python, Java).         |
| **`cognitiveService`**  | `runConsistencyCheck(model)`                           | Envoie le modèle à un module WebAssembly (WASM) pour analyse.     |

---

## 🧠 Gestion d'État (Stores)

L'application utilise **Zustand** pour partager l'état entre les pages sans complexité.

- **`useModelStore`** : C'est le cœur de l'application. Il contient l'objet `ProjectModel` complet (Arbre Arcadia).
- **`useSettingsStore`** : Persiste les configurations utilisateur (Choix du backend IA, Chemins DB).
- **`useUiStore`** : Gère l'état de l'interface pure (Sidebar ouverte/fermée, Thème).
- **`useAiStore`** : Stocke l'historique de la conversation avec l'assistant.

## 🔄 Flux de Données Type (Exemple: GenRules)

Voici le cycle de vie d'une donnée lorsqu'un utilisateur modifie une facture dans la démo :

1.  **UI Event** : L'utilisateur tape `10` dans le champ "Jours" (`InvoiceDemo.tsx`).
2.  **Hook** : `useRulesEngine` détecte le changement et lance un timer (Debounce 500ms).
3.  **Tauri Bridge** : Le hook appelle `invoke('jsondb_evaluate_draft', { doc })`.
4.  **Rust Backend** :
    - Le `CollectionsManager` charge le schéma JSON.
    - L'`Evaluator` exécute les règles (Maths, Lookup DB).
    - Rust renvoie le document enrichi (Total calculé).
5.  **React Update** : Le hook reçoit le résultat et met à jour le state local.
6.  **Render** : `InvoiceDemo` ré-affiche le total et la date d'échéance.

## 🚀 Commandes de Développement

```bash
# Installer les dépendances JS
npm install

# Lancer le serveur de dev (avec Backend Rust)
# Cette commande compile Rust ET lance Vite en parallèle
cargo tauri dev

# Linter le code TypeScript
npm run lint

# Construire pour la production (Génère l'exécutable final)
cargo tauri build
```
