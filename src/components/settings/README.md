# ⚙️ Paramètres Système (Settings)

Ce répertoire contient l'interface de configuration globale de **RAISE**. Cette page permet à l'utilisateur de définir comment le frontend React interagit avec les services backend (IA et Base de Données).

## 📂 Structure des Fichiers

```text
src/components/settings/
├── SettingsPage.tsx        // Page principale de configuration
└── README.md               // Documentation (ce fichier)
```

---

## 🧩 Composant : `SettingsPage`

Le composant `SettingsPage` offre une interface utilisateur pour modifier le `SettingsStore` et tester la connectivité avec le backend Rust.

### 🚀 Fonctionnalités

#### 1\. Configuration de l'IA

Cette section permet de choisir le moteur d'intelligence artificielle utilisé par les agents et le chat.

- **Options de Backend** :
  - `Mock` : Simulation (pas d'appels réels, utile pour le développement UI).
  - `Tauri Local` : Utilise un modèle LLM local (via llama.cpp ou Rust interne).
  - `Remote API` : Connecte à des APIs externes (OpenAI, Mistral) via HTTPS.

#### 2\. Base de Données (JSON-DB)

Cette section configure la cible de stockage pour les modèles Arcadia.

- **Espace (Space)** : Le namespace logique (ex: `demo_space`).
- **Base (Database)** : Le nom de la base de données (ex: `demo_db`).
- **Action "Tester & Recharger"** :
  - Déclenche `modelService.loadProjectModel` avec les paramètres saisis.
  - Si la connexion Rust réussit, le `ModelStore` est mis à jour avec le nouveau projet chargé.
  - Affiche un message de succès (vert) ou d'erreur (rouge) pour informer l'utilisateur de l'état de la connexion.

### 🎨 Styles

Le composant utilise des styles in-line (variables CSS CSS-in-JS like) pour s'adapter au thème de l'application (`var(--bg-panel)`, `var(--text-main)`, etc.).

---

## 🔄 Flux de Données

1.  **Modification** : Lorsqu'un utilisateur change une valeur (ex: Backend IA), le `SettingsStore` est immédiatement mis à jour.
2.  **Action** : Le clic sur "Tester & Recharger" lance un appel asynchrone vers Tauri via le `modelService`.
3.  **Résultat** :
    - **Succès** : Le projet est chargé dans le `ModelStore` et l'interface affiche le nom du projet.
    - **Erreur** : L'exception est parsée (`parseError`) et affichée à l'utilisateur (ex: "Backend Rust non lancé").
