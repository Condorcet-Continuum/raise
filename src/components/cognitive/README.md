# Module Cognitive Analysis 🧠

Ce module est l'interface de contrôle du **Moteur Cognitif** de RAISE.
Sa fonction est de soumettre le modèle d'architecture actif à des algorithmes d'analyse avancés (vérification de cohérence, détection de conflits sémantiques) exécutés par le backend Rust via des plugins **WebAssembly (WASM)**.

---

## 📂 Structure du dossier

| Fichier                     | Rôle                                                                                                                                  |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| **`CognitiveAnalysis.tsx`** | **Composant Maître**. Gère la transformation du modèle, l'appel au service, l'état de chargement et l'affichage du rapport d'analyse. |

---

## 🏗️ Architecture Technique

Ce module ne contient pas la logique d'analyse elle-même. Il agit comme un **client riche** pour le backend.

### Flux de Données (Data Flow) :

1.  **Extraction :** Le composant récupère le projet Arcadia complet depuis le `model-store`.
2.  **Transformation :** Il convertit ce modèle spécifique (OA/SA/LA/PA) en un format pivot générique (`CognitiveModel`) défini dans `src/types/cognitive.types.ts`.
    - _Pourquoi ?_ Pour que les plugins WASM soient agnostiques de la structure interne complexe d'Arcadia.
3.  **Transmission :** Envoi de la payload JSON au backend Rust via `cognitiveService`.
4.  **Exécution (Backend) :** Rust charge le fichier `.wasm` (ex: `consistency_basic.wasm`), lui passe les données, et récupère la sortie.
5.  **Rendu :** Le composant affiche le `AnalysisReport` retourné (Score, Statut, Liste des anomalies).

---

## 🎨 Design & Thèmes

L'interface est divisée en deux colonnes pour une lisibilité maximale :

- **Colonne Gauche (Rapport) :**
  - Affiche les messages détaillés du plugin.
  - Utilise des couleurs contextuelles : Vert (Succès), Orange (Avertissement), Rouge (Erreur).
  - Affiche l'ID du bloc WASM exécuté pour la traçabilité.
- **Colonne Droite (Synthèse) :**
  - Affiche le **Score Global** (/100) et le **Statut**.
  - Contient le bouton d'action principal ("Exécuter l'analyse").

---

## 💻 Exemple d'intégration

```tsx
import CognitiveAnalysis from '@/components/cognitive/CognitiveAnalysis';

export default function CognitivePage() {
  return (
    <div style={{ height: '100%', overflowY: 'auto' }}>
      <CognitiveAnalysis />
    </div>
  );
}
```

---

## 🛠️ Maintenance

- **Ajout de propriétés :** Si le plugin WASM a besoin de nouvelles données (ex: propriétés physiques des composants), il faut mettre à jour la fonction `transformToCognitiveModel` dans `CognitiveAnalysis.tsx` et l'interface `ModelElement` dans les types.
- **Gestion d'erreurs :** Le composant gère les erreurs de sérialisation (JS) et les erreurs d'exécution WASM (Rust) via un bloc `try/catch` robuste et un affichage d'erreur visuel.

<!-- end list -->

```

```
