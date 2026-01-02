# 📊 Tableau de Bord (Dashboard)

Ce répertoire contient la vue principale de l'application **RAISE**. C'est l'écran d'accueil qui s'affiche au démarrage, offrant une vue synthétique de l'état du système et du projet en cours.

## 📂 Structure des Fichiers

```text
src/components/dashboard/
├── DashboardView.tsx       // Composant principal d'affichage
└── README.md               // Documentation (ce fichier)
```

---

## 🧩 Composant : `DashboardView`

Le `DashboardView` est un composant de présentation qui agrège des données provenant de plusieurs sources (Store global, Backend Rust, Props) pour donner un feedback immédiat à l'utilisateur.

### 📋 Props & Interface

Le composant attend les propriétés suivantes pour fonctionner :

| Prop             | Type                     | Description                                                                               |
| :--------------- | :----------------------- | :---------------------------------------------------------------------------------------- |
| **`sysInfo`**    | `any` (Object)           | Informations système retournées par le backend Rust (version, environnement, chemins DB). |
| **`onNavigate`** | `(page: string) => void` | Fonction de rappel pour changer la page active dans `App.tsx`.                            |

### 🚀 Fonctionnalités

#### 1\. Indicateurs Clés (KPIs)

Affiche trois cartes principales résumant l'état de l'application :

- **Projet Actif** : Nom et description du projet chargé (via `useModelStore`).
- **Éléments** : Nombre d'objets chargés en mémoire.
- **Moteur IA** : Statut de la connexion avec le backend.

#### 2\. Statut Système (Backend Rust)

Si la connexion avec Tauri est établie (`sysInfo` non null), un panneau vert s'affiche avec les détails techniques :

- **Version** de l'application.
- **Mode** (Debug/Release).
- **Chemin** de la base de données JSON.

#### 3\. Actions Rapides

Fournit des boutons d'accès direct aux fonctionnalités clés sans passer par le menu latéral :

- **Paramètres** (`Settings`).
- **Moteur de Règles** (`Rules Engine Demo`).

### 🔌 Intégration

Ce composant est conçu pour être instancié par le layout principal (`App.tsx`) qui lui injecte les données système récupérées au démarrage via Tauri (`invoke`).

**Exemple d'utilisation :**

```tsx
// Dans App.tsx
import DashboardView from '@/components/dashboard/DashboardView';

// ...
<DashboardView sysInfo={sysInfo} onNavigate={setCurrentPage} />;
```

## 🛠️ Sous-composants Locaux

Pour garder le code propre, certains petits composants de présentation sont définis localement dans `DashboardView.tsx` :

- **`DashboardCard`** : Structure visuelle standardisée pour les KPIs.
- **`ActionButton`** : Bouton stylisé pour les liens rapides.
