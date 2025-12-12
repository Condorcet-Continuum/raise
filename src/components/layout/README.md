# Layout Components 📐

Ce répertoire contient les composants structurels ("Scaffolding") de l'application.
Leur rôle n'est pas de gérer la logique métier, mais de définir le squelette visuel (Header, Sidebar, Zone de contenu) qui entoure les pages.

---

## 📂 Inventaire des Composants

| Fichier              | Rôle                                                                                                                                                          |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`MainLayout.tsx`** | **Wrapper Principal**. C'est le composant parent de toutes les pages. Il positionne la Sidebar à gauche (fixe) et le contenu principal à droite (scrollable). |
| **`Sidebar.tsx`**    | Menu de navigation vertical. Contient les liens vers les différents modules (Modélisation, IA, Génétique, etc.).                                              |
| **`Header.tsx`**     | Barre supérieure horizontale. Affiche le titre de la page courante et contient le bouton de bascule de thème (Dark/Light).                                    |

---

## 🎨 Design & Thèmes

Ces composants définissent la structure visuelle globale de GenAptitude.

### Dimensions Clés (Variables CSS)

- **Largeur Sidebar :** `var(--sidebar-width)` (ex: 280px).
- **Hauteur Header :** `var(--header-height)` (ex: 64px).

### Couleurs Structurelles

- **Fond Sidebar/Header :** `var(--bg-panel)` (Blanc ou Gris foncé).
- **Fond Zone Contenu :** `var(--bg-app)` (Gris très clair ou Noir bleuté).
- **Bordures :** `var(--border-color)` assure une séparation subtile entre les zones.

---

## 💻 Fonctionnement du Layout

Le `MainLayout` utilise **Flexbox** pour gérer l'espace :

1.  **Conteneur global (`100vh`) :** `display: flex`.
2.  **Sidebar :** Largeur fixe, hauteur 100%.
3.  **Zone Droite (`flex: 1`) :** Colonne verticale contenant :
    - **Header :** Hauteur fixe.
    - **Main (`flex: 1`) :** Occupe tout l'espace restant. C'est ici que `overflow-y: auto` est appliqué pour permettre le scroll du contenu sans scroller toute la page (la Sidebar reste fixe).

```tsx
<MainLayout
  currentPage="dashboard"
  pageTitle="Tableau de bord"
  onNavigate={(page) => setPage(page)}
>
  {/* Le contenu de la page est injecté ici (children) */}
  <DashboardContent />
</MainLayout>
```

## 🛠️ Maintenance

- **Ajout d'une page :**
  1.  Ajoutez l'entrée dans le tableau `menuItems` de `Sidebar.tsx`.
  2.  Ajoutez le cas correspondant dans le `switch` de `App.tsx`.
- **Responsive :** Actuellement conçu pour Desktop. Pour le mobile, il faudrait ajouter un état `isOpen` dans le `ui-store` pour masquer/afficher la Sidebar.

<!-- end list -->

```

```
