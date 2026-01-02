# Styles & Theming 🎨

Ce répertoire contient l'ensemble des définitions graphiques de RAISE.
L'architecture repose sur les **Variables CSS natives** (Custom Properties) pour permettre un changement de thème instantané sans rechargement de page (via l'attribut `data-theme` sur la racine HTML).

---

## 📂 Structure des fichiers

| Fichier                | Rôle                                                                                                                                                                                  |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`globals.css`**      | **Point d'entrée**. Il importe les autres fichiers, applique le Reset CSS standard et définit les styles globaux (body, scrollbar). C'est le seul fichier CSS importé dans `App.tsx`. |
| **`variables.css`**    | Contient les variables **structurelles** qui ne changent pas selon le thème : Polices, Tailles, Espacements (`--spacing-*`), Arrondis (`--radius-*`).                                 |
| **`themes/light.css`** | Définit la palette de couleurs pour le **Mode Clair** (activé par défaut).                                                                                                            |
| **`themes/dark.css`**  | Définit la palette de couleurs pour le **Mode Sombre** (activé via `[data-theme='dark']`).                                                                                            |

---

## 🌈 Architecture des Couleurs

Nous n'utilisons **jamais** de codes hexadécimaux (`#ffffff`, `#000000`) directement dans les composants React. Nous utilisons des **variables sémantiques**.

### Variables Sémantiques Clés

| Variable          | Usage                                                 | Light Value     | Dark Value          |
| ----------------- | ----------------------------------------------------- | --------------- | ------------------- |
| `--bg-app`        | Fond global de l'application (derrière les panneaux). | Gris très clair | Noir bleuté profond |
| `--bg-panel`      | Fond des cartes, sidebars, modales.                   | Blanc           | Gris foncé          |
| `--text-main`     | Texte principal.                                      | Gris foncé      | Blanc cassé         |
| `--text-muted`    | Texte secondaire, labels, métadonnées.                | Gris moyen      | Gris moyen          |
| `--border-color`  | Bordures de séparation.                               | Gris clair      | Gris sombre         |
| `--color-primary` | Action principale, liens, focus.                      | Indigo          | Indigo (ajusté)     |

---

## 💻 Guide d'utilisation

### 1. Dans un fichier CSS

```css
.ma-classe {
  /* Utiliser les variables pour tout */
  padding: var(--spacing-4);
  background-color: var(--bg-panel);
  color: var(--text-main);
  border-radius: var(--radius-md);
}
```

### 2\. Dans un composant React (Style inline)

```tsx
<div
  style={{
    backgroundColor: 'var(--bg-panel)',
    border: '1px solid var(--border-color)',
    color: 'var(--text-main)',
  }}
>
  Contenu compatible Dark Mode
</div>
```

---

## 🌗 Mécanisme du Dark Mode

Le basculement se fait via le composant `src/components/shared/ThemeToggle.tsx`.

1.  Au clic, il modifie l'attribut sur la racine : `<html data-theme="dark">`.
2.  Le fichier `themes/dark.css` contient un sélecteur `[data-theme='dark']` qui écrase les variables de couleurs.
3.  Grâce à la transition CSS définie dans `globals.css` (`transition: background-color 0.3s`), le changement est fluide.

<!-- end list -->

```css
/* Extrait de globals.css */
body {
  background-color: var(--bg-app); /* Change dynamiquement */
  color: var(--text-main); /* Change dynamiquement */
  transition: background-color 0.3s ease, color 0.3s ease;
}
```

---

## 🛠️ Maintenance

Pour ajouter une nouvelle couleur :

1.  Déclarez la variable dans `themes/light.css`.
2.  Déclarez la **même variable** (avec une valeur adaptée) dans `themes/dark.css`.
3.  Utilisez la variable partout dans l'application.

<!-- end list -->

```

```
