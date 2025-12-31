# UI Components Library 🧩

Ce répertoire contient l'intégralité des interfaces utilisateur de RAISE.
L'architecture suit une approche **modulaire** : chaque dossier représente un domaine fonctionnel distinct, à l'exception de `shared` (générique) et `layout` (structurel).

Certains composants racines (comme les Testeurs) sont situés directement à la racine de `components/` pour un accès rapide aux outils de diagnostic.

---

## 🌳 Arborescence Complète

Voici la structure actuelle des composants du projet :

```text
src/components/
├── ai-chat/                  # 🤖 Interface conversationnelle (LLM)
│   ├── ChatInterface.tsx     # Composant principal
│   ├── ContextDisplay.tsx    # Méta-données de session
│   ├── InputBar.tsx          # Zone de saisie
│   ├── IntentClassifier.tsx  # Détection d'intention
│   ├── MessageBubble.tsx     # Bulle de message (User/AI)
│   └── SuggestionPanel.tsx   # Chips de suggestions
│
├── assurance/                # 🛡️ Tableau de bord Qualité & XAI
│   └── AssuranceDashboard.tsx
│
├── blockchain/               # 🔗 Notifications et visualisations Ledger
│   ├── BlockchainToast.tsx   # Notification style "Matrix"
│   └── BlockchainView.tsx    # Vue de démo Ledger (Refactorisé)
│
├── code-editor/              # 💻 Éditeur léger pour JSON/Scripts
│   ├── CodeCompletion.tsx    # Popup d'autocomplétion
│   ├── CodeEditor.tsx        # Wrapper Textarea avec lignes
│   ├── LivePreview.tsx       # Panneau de rendu JSON
│   └── SyntaxHighlighter.tsx # Coloration syntaxique simple
│
├── codegen/                  # ⚙️ Usine logicielle
│   └── CodeGenerator.tsx     # Interface de génération de sources
│
├── cognitive/                # 🧠 Moteur Cognitif
│   └── CognitiveAnalysis.tsx # Vue principale analyse
│
├── dashboard/                # 📊 Vue d'accueil (Refactorisé)
│   └── DashboardView.tsx     # KPIs et Infos Système
│
├── diagram-editor/           # ✏️ Canvas de modélisation visuelle
│   ├── ConnectionTool.tsx    # Barre d'outils flottante
│   ├── DiagramCanvas.tsx     # Zone de dessin infinie
│   ├── LayoutEngine.tsx      # Panneau d'auto-layout
│   └── ShapeLibrary.tsx      # Sidebar des formes (Drag&Drop)
│
├── genetics/                 # 🧬 Dashboard d'optimisation
│   └── GeneticsDashboard.tsx # Configuration et graphiques
│
├── layout/                   # 📐 Structure globale de l'application
│   ├── Header.tsx            # Barre supérieure
│   ├── MainLayout.tsx        # Wrapper principal (Flexbox)
│   └── Sidebar.tsx           # Navigation latérale
│
├── model-viewer/             # 💠 Explorateur de modèles Arcadia
│   ├── ArcadiaLayerView.tsx  # Sélecteur de couches (OA/SA/LA...)
│   ├── CapellaViewer.tsx     # Vue principale (SplitPane)
│   ├── DataDictionary.tsx    # Vue liste des éléments
│   ├── DiagramRenderer.tsx   # Placeholder de rendu graphique
│   ├── ElementInspector.tsx  # Panneau de propriétés
│   └── ModelNavigator.tsx    # Arbre du projet
│
├── rules_engine/             # 🧮 Moteur de Règles Réactif (GenRules)
│   ├── InvoiceDemo.tsx       # Démo Facturation (Calculs & Lookup)
│   ├── ModelRulesDemo.tsx    # Démo Ingénierie (Validation & Naming)
│   └── RulesEngineDashboard.tsx # Conteneur de navigation
│
├── settings/                 # ⚙️ Configuration Système
│   └── SettingsPage.tsx      # Paramètres IA & DB
│
├── shared/                   # 🧱 Design System (Composants atomiques)
│   ├── Button.tsx            # Bouton standard
│   ├── Card.tsx              # Conteneur générique
│   ├── Modal.tsx             # Fenêtre modale
│   ├── SplitPane.tsx         # Diviseur d'écran resizable
│   ├── Tabs.tsx              # Navigation par onglets
│   ├── ThemeToggle.tsx       # Switch Dark/Light mode
│   └── TreeView.tsx          # Composant d'arbre récursif
│
├── workflow-designer/        # 🔀 Orchestrateur de pipelines CI/CD
│   ├── ConnectionManager.tsx # Rendu des liens (SVG)
│   ├── ExecutionMonitor.tsx  # Console de logs
│   ├── NodeLibrary.tsx       # Sidebar des tâches
│   └── WorkflowCanvas.tsx    # Zone de travail Node-based
│
# --- COMPOSANTS RACINES (OUTILS DIAGNOSTIC) ---
├── CognitiveTester.tsx       # 🧪 Testeur du moteur WASM (Consistency)
└── JsonDbTester.tsx          # 🗄️ Explorateur Bas Niveau JSON-DB (CRUD/Search)
```

---

## 🧭 Guide de Navigation

Chaque sous-dossier contient son propre `README.md` détaillé expliquant :

- Le rôle précis du module.
- Ses dépendances.
- Des exemples d'intégration.

### Catégories de composants

1.  **Structurels (`layout/`)** :
    Définissent le cadre de l'application. Ils ne sont utilisés qu'une seule fois, au niveau de `App.tsx`.

2.  **Atomiques (`shared/`)** :
    Les briques de base (Boutons, Inputs). Ils doivent être **purs** (pas de logique métier complexe) et réutilisables partout.

3.  **Métiers (Les dossiers thématiques)** :
    Contiennent la logique spécifique à une fonctionnalité (ex: `rules_engine` dialogue avec le backend Rust pour les calculs, `model-viewer` connaît le format Arcadia).

4.  **Outils de Diagnostic (Racine)** :

    - **`JsonDbTester.tsx`** : Interface d'administration brute pour la base de données (Créer/Supprimer DB, Requêtes, Index).
    - **`CognitiveTester.tsx`** : Interface de test pour le chargement dynamique de modules WASM et l'analyse de cohérence sur des données réelles ou simulées.

---

## ⚠️ Règles de contribution

- **Styles :** N'utilisez jamais de CSS global ou de classes arbitraires. Utilisez les variables définies dans `src/styles/variables.css` pour garantir le support du **Dark Mode**.
- **Dépendances :** Un composant "Métier" peut utiliser des composants "Shared". Un composant "Shared" ne doit jamais importer un composant "Métier".
- **État :** Si un composant a besoin d'accéder à l'état global (ex: Projet chargé), utilisez les Hooks personnalisés (`useModelStore`, `useSettingsStore`) plutôt que de propager les props sur 10 niveaux.
