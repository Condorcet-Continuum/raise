# Module Product Assurance 🛡️

Ce module est dédié à la **Gouvernance et à la Confiance**.
Dans un contexte d'ingénierie système assistée par IA, il est crucial de ne pas seulement générer des modèles, mais de garantir qu'ils respectent les standards de qualité (QA) et de comprendre pourquoi l'IA a fait certains choix (XAI).

---

## 📂 Structure du dossier

| Fichier                      | Rôle                                                                                                               |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| **`AssuranceDashboard.tsx`** | **Composant Maître**. Tableau de bord unifié proposant deux vues commutables (Onglets) : Qualité et Explicabilité. |

---

## 🎨 Design & Thèmes

Le dashboard utilise une approche visuelle rassurante, basée sur des indicateurs clairs (KPIs).

### Adaptation chromatique :

- **Indicateurs de Santé (KPIs) :**
  - Utilise `var(--color-success)` (Vert) pour les métriques conformes (ex: Code Coverage > 90%).
  - Utilise `var(--color-warning)` (Orange) pour la dette technique ou les avertissements.
  - Utilise `var(--color-info)` (Bleu) pour les métriques informatives (Complexité).
- **Cartes XAI :**
  - Fond contrasté par rapport au panneau (`var(--bg-app)` sur `var(--bg-panel)`) pour mettre en avant la décision de l'IA.
  - Bordure latérale (`border-left`) colorée pour identifier la sévérité de la justification.

---

## 💻 Fonctionnalités

### 1. Vue Qualité (QA)

Affiche des métriques simulées (pour l'instant) concernant la santé du projet :

- **Couverture de Code/Modèle :** Pourcentage d'éléments validés.
- **Complexité Cyclomatique :** Indice de complexité structurelle du modèle.
- **Dette Technique :** Estimation du temps nécessaire pour refactoriser.

### 2. Vue Explicabilité (XAI)

C'est le module de **Transparence**.
L'IA générative (GenAI) peut parfois agir comme une "boîte noire". Cette vue affiche les logs de raisonnement (Chain-of-Thought) qui justifient les choix architecturaux.

_Exemple : "Pourquoi avoir choisi un pattern Control Loop ?" -> "Parce que la latence requise est < 10ms."_

---

## 💻 Exemple d'intégration

Ce composant est conçu pour être une page principale.

```tsx
import AssuranceDashboard from '@/components/assurance/AssuranceDashboard';

export default function AssurancePage() {
  return (
    <div style={{ height: '100%', overflowY: 'auto' }}>
      <AssuranceDashboard />
    </div>
  );
}
```

---

## 🛠️ Évolutions possibles

- **Connecteur SonarQube :** Remplacer les données QA simulées par des appels API vers un serveur SonarQube ou un outil d'analyse statique Rust.
- **Traceabilité des Exigences :** Lier chaque métrique QA à une exigence système (ReqIF).
- **Arbre de Décision XAI :** Visualiser graphiquement le cheminement de l'IA plutôt que sous forme de texte plat.

<!-- end list -->

```

```
