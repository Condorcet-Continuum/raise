# Module Blockchain 🔗

Ce module contient les composants d'interface liés aux fonctionnalités **Blockchain / Ledger** de RAISE (ex: notarisation, ancrage de preuves).
Actuellement, il fournit principalement des retours visuels (Toasts) stylisés pour signaler des événements de consensus.

---

## 📂 Structure du dossier

| Fichier                   | Rôle                                                                                                                      |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| **`BlockchainToast.tsx`** | Composant de notification "Toast" qui apparaît en bas à droite pour confirmer un ancrage Blockchain (Hyperledger Fabric). |

---

## 🎨 Design & Thèmes

Le composant `BlockchainToast` possède une identité visuelle distincte ("Tech/Security") tout en restant compatible avec le thème global de l'application.

### Spécificités visuelles :

- **Typographie :** Utilise `var(--font-family-mono)` (Consolas/Monospace) pour renforcer l'aspect technique et cryptographique.
- **Palette de couleurs :**
  - Repose massivement sur la sémantique **Success** (`var(--color-success)`) pour évoquer la validation.
  - **Mode Clair :** Carte blanche avec bordure verte et texte sombre.
  - **Mode Sombre :** Carte sombre (`var(--bg-panel)`) avec bordure verte lumineuse (Effet "Matrix" modernisé).
- **Animations :**
  - `slideUp` : Entrée fluide depuis le bas de l'écran.
  - `pulse-success` : Effet de battement (aura) utilisant la couleur de succès définie dans le thème.

---

## 💻 Exemple d'intégration

Le composant s'utilise généralement dans un layout global ou une page spécifique, déclenché par un état booléen.

```tsx
import { useState } from 'react';
import { BlockchainToast } from '@/components/blockchain/BlockchainToast';

export function TransactionPage() {
  const [showToast, setShowToast] = useState(false);

  const handleTransaction = () => {
    // Logique métier...
    // Une fois terminé :
    setShowToast(true);

    // Note : Le composant gère sa propre disparition automatique après 8 secondes.
  };

  return (
    <div>
      <button onClick={handleTransaction}>Valider le Bloc</button>

      {/* Le Toast se positionne en fixed, peu importe où il est déclaré */}
      <BlockchainToast trigger={showToast} />
    </div>
  );
}
```

---

## 🛠️ Comportement

1.  **Trigger :** Le composant surveille la prop `trigger`. Lorsqu'elle passe à `true`, le Toast devient visible.
2.  **Auto-Dismiss :** Un timer interne masque automatiquement la notification après **8 secondes**.
3.  **Z-Index :** Utilise `var(--z-index-tooltip)` pour s'assurer d'être toujours au-dessus des autres éléments (Sidebars, Modales).

## 🚀 Évolutions possibles

- Ajouter des variantes pour les erreurs (échec de consensus) ou les chargements (mining en cours).
- Passer les données (Hash, ID de transaction) en props dynamiques plutôt qu'en dur.

<!-- end list -->

```

```
