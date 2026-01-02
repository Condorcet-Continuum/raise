# Module Genetics (Optimisation) 🧬

Ce module propose une interface de tableau de bord pour piloter les algorithmes d'optimisation génétique de RAISE.
Il permet de configurer les hyperparamètres de la simulation (population, mutation), de lancer l'évolution et de visualiser la convergence des résultats en temps réel.

---

## 📂 Structure du dossier

| Fichier                     | Rôle                                                                                                                                 |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| **`GeneticsDashboard.tsx`** | **Composant Unique**. Contient à la fois le panneau de configuration (gauche) et le panneau de visualisation des résultats (droite). |

---

## 🎨 Design & Thèmes

Ce module se distingue par une identité visuelle **"Organique & Scientifique"**, utilisant principalement les tons violets et roses (`--color-accent`), tout en restant parfaitement lisible en mode clair ou sombre.

### Adaptation chromatique :

- **Couleur Dominante :** `var(--color-accent)` (Violet/Mauve). Utilisée pour les titres, les curseurs (sliders) et les éléments graphiques.
- **Bouton d'Action :** Utilise un dégradé dynamique `linear-gradient(90deg, var(--color-accent), var(--color-primary))` pour attirer l'attention sur l'action principale "Lancer l'Optimisation".
- **Graphique de Convergence :**
  - Les barres sont générées en CSS pur (`backgroundColor: var(--color-accent)`).
  - L'opacité (0.8) permet de garder une légèreté visuelle sur les fonds sombres.
- **Statistiques :**
  - Score : `var(--color-success)` (Vert).
  - Durée : `var(--color-info)` (Bleu).
  - ID Candidat : `var(--color-accent)` (Violet).

---

## 💻 Fonctionnalités

1.  **Configuration des Paramètres :**

    - **Taille de la Population :** Nombre d'individus par génération.
    - **Générations :** Nombre d'itérations de l'algorithme.
    - **Taux de Mutation :** Probabilité de modification aléatoire d'un gène.
    - _Note : Les inputs utilisent `accentColor` en CSS pour s'aligner sur le thème._

2.  **Visualisation des Résultats :**

    - Affichage des métriques clés (Meilleur score, Temps d'exécution).
    - **Graphique CSS :** Un histogramme simple montrant la progression du score d'adaptation (fitness) au fil des générations.

3.  **Intégration Service :**
    - Appelle `geneticsService.runOptimization()` de manière asynchrone.
    - Gère l'état de chargement avec une animation CSS (`pulse`).

---

## 💻 Exemple d'intégration

```tsx
import GeneticsDashboard from '@/components/genetics/GeneticsDashboard';

export default function OptimizationPage() {
  return (
    <div style={{ height: '100%' }}>
      <GeneticsDashboard />
    </div>
  );
}
```

---

## 🛠️ Évolutions possibles

- **Graphiques Avancés :** Remplacer le graphique CSS par une librairie dédiée (Recharts ou Chart.js) pour afficher des courbes de tendances plus précises.
- **Visualisation du Candidat :** Afficher un aperçu (JSON ou Diagramme) de la solution architecturale gagnante.
- **Comparaison :** Permettre de lancer plusieurs simulations en parallèle pour comparer l'impact du taux de mutation.
