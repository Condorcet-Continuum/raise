# Utility Functions Library 🛠️

Ce répertoire contient une collection de fonctions utilitaires **pures**, **stateless** et **génériques**.
Elles sont utilisées à travers toute l'application pour simplifier le code métier et garantir la cohérence des formats (dates, nombres, chaînes).

---

## 📂 Inventaire des Utilitaires

| Fichier             | Rôle                                                               | Fonctions Clés                                                     |
| ------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------ |
| **`helpers.ts`**    | Fonctions d'aide générale et manipulation DOM/CSS.                 | `cn` (ClassNames), `sleep`, `debounce`, `generateId`, `deepClone`  |
| **`formatters.ts`** | Transformation de données brutes en format lisible pour l'humain.  | `formatDate`, `formatFileSize`, `formatArcadiaType`, `truncate`    |
| **`validators.ts`** | Vérification de l'intégrité des données (retourne booléen).        | `isValidJson`, `isEmpty`, `isUuid`, `hasProperties`                |
| **`converters.ts`** | Transformation de structure de données ou de format technique.     | `arrayToRecord` (Array -> Map), `hexToRgba`, `camelToSnakeCase`    |
| **`parsers.ts`**    | Extraction et nettoyage de données depuis des sources incertaines. | `parseError` (Safe try/catch), `getFileExtension`, `safeJsonParse` |

---

## 💻 Guide d'utilisation

### 1. Gestion des Classes CSS (`cn`)

Inspiré de la librairie `clsx` ou `classnames`, permet de concaténer des classes conditionnelles.

```typescript
import { cn } from '@/utils/helpers';

// Si isActive est true : "btn btn-primary active"
// Si isActive est false : "btn btn-primary"
<button className={cn('btn', 'btn-primary', isActive && 'active')} />;
```

### 2\. Formatage de Dates (`formatDate`)

Standardise l'affichage des dates dans toute l'application (Format FR).

```typescript
import { formatDate } from '@/utils/formatters';

// Affiche : "05/10/2023 14:30"
<span>{formatDate(message.createdAt)}</span>;
```

### 3\. Gestion des Erreurs (`parseError`)

Permet d'afficher un message d'erreur propre, quel que soit le type de l'exception levée (String, Error, Object).

```typescript
import { parseError } from '@/utils/parsers';

try {
  await apiCall();
} catch (err) {
  // Affiche toujours une string lisible
  console.error(parseError(err));
}
```

### 4\. Optimisation des Stores (`arrayToRecord`)

Transforme un tableau en objet indexé par ID pour des recherches en O(1).

```typescript
import { arrayToRecord } from '@/utils/converters';

const users = [
  { id: 'u1', name: 'Alice' },
  { id: 'u2', name: 'Bob' },
];
const userMap = arrayToRecord(users, 'id');

// Résultat : { 'u1': { id: 'u1'... }, 'u2': { id: 'u2'... } }
console.log(userMap['u1']);
```

---

## ⚠️ Bonnes Pratiques

1.  **Fonctions Pures :** Les utilitaires ne doivent pas modifier leurs arguments (immutabilité) ni dépendre d'un état global (Store, DOM).
2.  **Pas de Logique Métier :** Si une fonction contient des règles métier spécifiques à Arcadia ou à l'IA, elle doit aller dans un `Service` ou un `Hook`, pas ici.
3.  **Typage Strict :** Utilisez les Generics TypeScript (`<T>`) autant que possible pour conserver le typage à la sortie de la fonction.
