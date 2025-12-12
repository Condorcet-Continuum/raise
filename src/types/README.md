# TypeScript Definitions 📐

Ce répertoire centralise toutes les interfaces et types TypeScript partagés dans l'application.
Il agit comme le **contrat de données** entre :

1.  Le Backend **Rust** (via Tauri IPC).
2.  Le Store global (**Zustand**).
3.  Les composants UI (**React**).

---

## 📂 Inventaire des Types

| Fichier                  | Rôle                                                                                                                                                                       |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`model.types.ts`**     | Définit la structure des projets d'architecture **Arcadia/SysML**. C'est ici que sont décrites les couches (OA, SA, LA, PA) et les éléments génériques (`ArcadiaElement`). |
| **`json-db.types.ts`**   | Contrat strict avec le moteur de base de données. Définit la syntaxe des requêtes (`Query`, `Filter`), des transactions et des documents génériques.                       |
| **`ai.types.ts`**        | Types pour le module d'Intelligence Artificielle : format des messages du chat, statuts du système LLM et résultats d'analyse NLP.                                         |
| **`cognitive.types.ts`** | Types d'échange avec le moteur d'analyse cognitive (WASM). Définit le format du rapport d'analyse (`AnalysisReport`).                                                      |
| **`arcadia.types.ts`**   | Fichier de **Constantes** (pas seulement des types) contenant les URIs et Namespaces officiels d'Arcadia (ex: `http://...#LogicalComponent`).                              |

---

## 🔗 Synchronisation Backend (Rust)

L'intégrité de l'application repose sur la correspondance exacte entre ces interfaces TypeScript et les `struct` Rust définies dans `src-tauri`.

**Exemple de correspondance :**

- **Rust (`json_db::Query`)** :
  ```rust
  pub struct Query {
      pub collection: String,
      pub limit: Option<usize>,
      // ...
  }
  ```
- **TypeScript (`src/types/json-db.types.ts`)** :
  ```typescript
  export interface Query {
    collection: string;
    limit?: number;
    // ...
  }
  ```

⚠️ **Attention :** Si vous modifiez une structure de données côté Rust, vous **devez** mettre à jour le fichier correspondant ici pour éviter des erreurs de désérialisation silencieuses.

---

## 💻 Guide d'utilisation

### 1. Importation

N'importez jamais les types depuis les fichiers de composants. Utilisez toujours les alias ou les chemins relatifs vers ce dossier.

```typescript
// ✅ Bon
import type { ProjectModel } from '@/types/model.types';

// ❌ Mauvais (Duplication locale)
interface ProjectModel { ... }
```

### 2\. Typage des Services

Utilisez ces types pour typer les retours des commandes `invoke`.

```typescript
import { invoke } from '@tauri-apps/api/core';
import type { QueryResponse } from '@/types/json-db.types';

const res = await invoke<QueryResponse>('my_command');
// res.documents est maintenant typé correctement !
```

### 3\. Utilisation des Constantes Arcadia

Pour éviter les "Magic Strings" lors du filtrage d'éléments.

```typescript
import { ArcadiaTypes, isArcadiaType } from '@/types/arcadia.types';

if (isArcadiaType(element.type, ArcadiaTypes.LA_COMPONENT)) {
  console.log("C'est un composant logique !");
}
```

---

## 🛠️ Maintenance

- **Extensions :** Si vous ajoutez un nouveau module métier (ex: Simulation Physique), créez un nouveau fichier `simulation.types.ts` plutôt que de surcharger `model.types.ts`.
- **Any :** L'utilisation de `any` est tolérée pour les propriétés dynamiques des modèles (`[key: string]: any`) car la structure exacte dépend du modèle utilisateur chargé en base.
