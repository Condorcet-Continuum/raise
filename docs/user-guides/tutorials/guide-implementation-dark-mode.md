# Guide d'Implémentation du Mode Dark
## GenAptitude - Use-case Factory

---

## 📋 Vue d'ensemble

Ce guide explique comment implémenter le mode dark dans votre application Tauri GenAptitude en utilisant les variables CSS fournies.

---

## 🎨 Philosophie du Design

### Mode Light
- **Usage :** Environnements lumineux, travail de jour
- **Objectif :** Clarté maximale, lecture confortable
- **Couleurs :** Fond clair (#F9FAFB), texte foncé (#1F2937)

### Mode Dark
- **Usage :** Environnements sombres, travail de nuit
- **Objectif :** Réduction de la fatigue oculaire, économie d'énergie
- **Couleurs :** Fond foncé (#111827), texte clair (#F3F4F6)

---

## 🚀 Mise en place rapide

### Étape 1 : Importer les variables CSS

```html
<!-- Dans votre index.html -->
<link rel="stylesheet" href="genaptitude-variables.css">
```

### Étape 2 : Initialiser le thème

```javascript
// main.js ou App.jsx
document.addEventListener('DOMContentLoaded', () => {
    // Récupérer le thème sauvegardé
    const savedTheme = localStorage.getItem('theme');
    
    // Ou détecter les préférences système
    const systemPrefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    
    // Définir le thème initial
    const initialTheme = savedTheme || (systemPrefersDark ? 'dark' : 'light');
    document.documentElement.setAttribute('data-theme', initialTheme);
});
```

### Étape 3 : Créer la fonction de toggle

```javascript
function toggleTheme() {
    const html = document.documentElement;
    const currentTheme = html.getAttribute('data-theme');
    const newTheme = currentTheme === 'dark' ? 'light' : 'dark';
    
    // Appliquer le nouveau thème
    html.setAttribute('data-theme', newTheme);
    
    // Sauvegarder dans localStorage
    localStorage.setItem('theme', newTheme);
    
    // Optionnel : Animer la transition
    document.body.style.transition = 'background-color 0.3s ease, color 0.3s ease';
}
```

---

## 🎛️ Composant Toggle React

### Exemple de composant ThemeToggle

```jsx
import React, { useState, useEffect } from 'react';

const ThemeToggle = () => {
    const [theme, setTheme] = useState('light');

    useEffect(() => {
        // Initialiser le thème au chargement
        const savedTheme = localStorage.getItem('theme');
        const systemPrefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
        const initialTheme = savedTheme || (systemPrefersDark ? 'dark' : 'light');
        
        setTheme(initialTheme);
        document.documentElement.setAttribute('data-theme', initialTheme);
        
        // Écouter les changements système
        const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
        const handleChange = (e) => {
            if (!localStorage.getItem('theme')) {
                const newTheme = e.matches ? 'dark' : 'light';
                setTheme(newTheme);
                document.documentElement.setAttribute('data-theme', newTheme);
            }
        };
        
        mediaQuery.addEventListener('change', handleChange);
        return () => mediaQuery.removeEventListener('change', handleChange);
    }, []);

    const toggleTheme = () => {
        const newTheme = theme === 'dark' ? 'light' : 'dark';
        setTheme(newTheme);
        document.documentElement.setAttribute('data-theme', newTheme);
        localStorage.setItem('theme', newTheme);
    };

    return (
        <button 
            onClick={toggleTheme}
            className="theme-toggle-button"
            aria-label={`Passer en mode ${theme === 'dark' ? 'clair' : 'sombre'}`}
        >
            {theme === 'dark' ? '🌙' : '☀️'}
        </button>
    );
};

export default ThemeToggle;
```

### Style du toggle

```css
.theme-toggle-button {
    background: var(--color-gray-200);
    border: none;
    padding: 12px;
    border-radius: 8px;
    cursor: pointer;
    font-size: 1.2em;
    transition: all 0.3s ease;
}

.theme-toggle-button:hover {
    background: var(--color-gray-300);
    transform: scale(1.1);
}

[data-theme="dark"] .theme-toggle-button {
    background: var(--color-gray-700);
}
```

---

## 🎨 Utilisation des Variables CSS

### Couleurs adaptatives

```css
/* ✅ BON - Utilise les variables qui s'adaptent */
.card {
    background: var(--color-gray-50);
    color: var(--color-gray-900);
    border: 1px solid var(--color-gray-200);
}

/* ❌ MAUVAIS - Couleurs codées en dur */
.card {
    background: #F9FAFB;
    color: #1F2937;
    border: 1px solid #E5E7EB;
}
```

### Dégradés

```css
/* Dégradés qui s'adaptent automatiquement */
.hero {
    background: var(--gradient-primary);
}

.accent {
    background: var(--gradient-accent);
}
```

### Ombres

```css
/* Les ombres sont plus prononcées en mode dark */
.card {
    box-shadow: var(--shadow-md);
}

.modal {
    box-shadow: var(--shadow-xl);
}

.button:hover {
    box-shadow: var(--shadow-primary);
}
```

---

## 🔧 Intégration Tauri

### Configuration Tauri (tauri.conf.json)

```json
{
  "tauri": {
    "windows": [
      {
        "title": "GenAptitude",
        "width": 1200,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600,
        "theme": "Auto"
      }
    ]
  }
}
```

### Détecter le thème système dans Tauri

```javascript
import { appWindow } from '@tauri-apps/api/window';

async function syncWithSystemTheme() {
    const theme = await appWindow.theme();
    document.documentElement.setAttribute('data-theme', theme || 'light');
}

// Écouter les changements de thème système
await appWindow.onThemeChanged(({ payload: theme }) => {
    if (!localStorage.getItem('theme')) {
        document.documentElement.setAttribute('data-theme', theme);
    }
});
```

---

## 📝 Bonnes Pratiques

### 1. Toujours utiliser les variables CSS

```css
/* ✅ Correct */
color: var(--color-gray-800);
background: var(--surface-primary);

/* ❌ À éviter */
color: #1F2937;
background: white;
```

### 2. Tester dans les deux modes

Vérifiez que tous les composants sont lisibles et fonctionnels dans les deux modes :
- Contraste suffisant (minimum 4.5:1 pour le texte)
- États de survol visibles
- Focus indicators clairement identifiables

### 3. Transitions fluides

```css
body {
    transition: background-color 0.3s ease, color 0.3s ease;
}

.card {
    transition: all 0.3s ease;
}
```

### 4. Préserver les préférences utilisateur

```javascript
// Toujours sauvegarder le choix de l'utilisateur
localStorage.setItem('theme', theme);

// Et le restaurer au chargement
const savedTheme = localStorage.getItem('theme');
```

---

## 🎯 Composants Spécifiques

### Boutons

```css
.btn-primary {
    background: var(--gradient-primary);
    color: var(--color-white);
    box-shadow: var(--shadow-md);
}

.btn-primary:hover {
    box-shadow: var(--shadow-primary);
    transform: translateY(-2px);
}
```

### Cartes

```css
.card {
    background: var(--surface-primary);
    border-radius: 12px;
    box-shadow: var(--shadow-md);
    padding: 24px;
    transition: all 0.3s ease;
}

.card:hover {
    box-shadow: var(--shadow-lg);
}
```

### Formulaires

```css
.input {
    background: var(--surface-secondary);
    border: 2px solid var(--color-gray-200);
    color: var(--color-gray-900);
    padding: 12px 16px;
    border-radius: 8px;
}

.input:focus {
    border-color: var(--color-primary);
    outline: none;
    box-shadow: 0 0 0 3px rgba(79, 70, 229, 0.1);
}

[data-theme="dark"] .input:focus {
    box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.2);
}
```

### Modales et Overlays

```css
.modal-backdrop {
    background: rgba(0, 0, 0, 0.5);
}

[data-theme="dark"] .modal-backdrop {
    background: rgba(0, 0, 0, 0.75);
}

.modal {
    background: var(--surface-primary);
    box-shadow: var(--shadow-2xl);
    border-radius: 12px;
}
```

---

## 🐛 Dépannage

### Le thème ne change pas

```javascript
// Vérifier que l'attribut est bien appliqué
console.log(document.documentElement.getAttribute('data-theme'));

// Vérifier que les variables CSS sont chargées
const style = getComputedStyle(document.documentElement);
console.log(style.getPropertyValue('--color-primary'));
```

### Les couleurs ne s'adaptent pas

```css
/* Vérifier la hiérarchie des sélecteurs */
[data-theme="dark"] .element {
    /* Styles dark mode */
}

/* S'assurer que le sélecteur est assez spécifique */
```

### Problèmes de performance

```javascript
// Désactiver les transitions pendant le changement de thème
document.body.style.transition = 'none';
document.documentElement.setAttribute('data-theme', newTheme);
setTimeout(() => {
    document.body.style.transition = '';
}, 0);
```

---

## 📊 Checklist d'Implémentation

- [ ] Variables CSS importées
- [ ] Fonction de toggle implémentée
- [ ] Sauvegarde dans localStorage
- [ ] Détection des préférences système
- [ ] Tous les composants testés en mode dark
- [ ] Transitions fluides
- [ ] Contraste suffisant (WCAG AA minimum)
- [ ] États de focus visibles
- [ ] Documentation mise à jour

---

## 🎨 Palette de Couleurs Référence

### Mode Light
| Usage | Variable | Valeur |
|-------|----------|--------|
| Texte principal | `--color-gray-800` | #1F2937 |
| Texte secondaire | `--color-gray-600` | #6B7280 |
| Arrière-plan | `--color-gray-50` | #F9FAFB |
| Surface | `--surface-primary` | #FFFFFF |
| Primary | `--color-primary` | #4F46E5 |

### Mode Dark
| Usage | Variable | Valeur |
|-------|----------|--------|
| Texte principal | `--color-gray-800` | #F3F4F6 |
| Texte secondaire | `--color-gray-600` | #D1D5DB |
| Arrière-plan | `--color-gray-50` | #111827 |
| Surface | `--surface-primary` | #1F2937 |
| Primary | `--color-primary` | #6366F1 |

---

## 📚 Ressources

- [Variables CSS complètes](genaptitude-variables.css)
- [Démo interactive](genaptitude-dark-mode-demo.html)
- [Charte graphique](genaptitude-charte-graphique.html)
- [WCAG Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)

---

## 💡 Conseils Finaux

1. **Testez régulièrement** : Basculez fréquemment entre les modes pendant le développement
2. **Accessibilité d'abord** : Le contraste doit être suffisant dans les deux modes
3. **Performance** : Utilisez `transition` avec parcimonie sur les éléments larges
4. **Cohérence** : Tous les composants doivent suivre les mêmes règles
5. **Feedback utilisateur** : Permettez facilement le changement de thème

---

**GenAptitude** - Use-case Factory  
Version 1.0 - 2025