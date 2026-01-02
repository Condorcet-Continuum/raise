# Module Rules Engine (GenRules)

Ce module implémente **GenRules**, le moteur de règles déclaratif et réactif de RAISE. Il permet de définir des logiques métier (calculs, validations, transformations) directement dans les schémas JSON, sans modifier le code compilé de l'application.

## 🏗️ Architecture

Le moteur est conçu pour être léger, sûr (pas d'exécution de code arbitraire) et intégrable au pipeline d'écriture de la base de données.

1.  **AST (`ast.rs`)** : Définit la grammaire des expressions (Maths, Logique, Dates, Strings, Lookup) sous forme d'arbre syntaxique abstrait sérialisable en JSON.
2.  **Evaluateur (`evaluator.rs`)** : Parcourt l'AST pour calculer le résultat final. Il gère les types, les erreurs et l'accès aux données externes via le trait `DataProvider`.
3.  **Analyseur (`analyzer.rs`)** : Inspecte statiquement une règle pour déterminer ses dépendances (quelles variables sont utilisées ?). Cela permet de construire le graphe de réactivité.
4.  **Store (`store.rs`)** : Stocke les règles en mémoire et maintient un index inversé (Champ -\> Règles impactées) pour déclencher uniquement les calculs nécessaires lors d'une mise à jour.

## 🚀 Fonctionnalités du Langage

Les expressions sont définies en JSON. Voici les capacités supportées par l'AST:

### 1\. Primitives et Variables

- `{"val": 42}` : Valeur littérale.
- `{"var": "user.age"}` : Lecture d'une variable du document courant (supporte la notation pointée).

### 2\. Mathématiques

- `add`, `sub`, `mul`, `div` : Opérations arithmétiques standard sur les nombres flottants.
- _Exemple_ : `{"mul": [{"var": "qty"}, {"var": "price"}]}`

### 3\. Logique et Contrôle

- `and`, `or`, `not` : Opérateurs booléens.
- `eq`, `neq`, `gt`, `gte`, `lt`, `lte` : Comparaisons.
- `if` : Structure conditionnelle `if / then / else`.

### 4\. Dates

- `now` : Date courante (ISO 8601).
- `date_diff` : Différence en jours entre deux dates.
- `date_add` : Ajout de jours à une date.

### 5\. Chaînes de Caractères

- `concat` : Concaténation de chaînes.
- `upper` : Conversion en majuscules.
- `regex_match` : Vérification par expression régulière.

### 6\. Lookups (Cross-Collection)

Permet de lire une valeur dans un **autre** document d'une autre collection.

- `lookup` : `{ "collection": "users", "id": "u1", "field": "email" }`.

## 🛠️ Intégration

Le moteur est principalement utilisé par le `CollectionsManager` de JSON-DB.

1.  **Chargement** : Au démarrage ou à l'insertion, les règles sont extraites de la propriété `x_rules` du schéma JSON.
2.  **Analyse** : L'`Analyzer` détecte que la règle R1 dépend de `price`.
3.  **Exécution** :
    - L'utilisateur modifie `price`.
    - Le `RuleStore` identifie que R1 doit être rejouée.
    - L'`Evaluator` exécute R1.
    - Si le résultat de R1 modifie `total`, et qu'une règle R2 dépend de `total`, R2 est déclenchée (propagation).

## 💻 Exemple de Règle JSON

Voici comment une règle est définie dans un fichier `.schema.json` :

```json
"x_rules": [
  {
    "id": "calc_total_ttc",
    "target": "billing.total_ttc",
    "expr": {
      "mul": [
        { "var": "billing.total_ht" },
        { "add": [1, { "var": "billing.tax_rate" }] }
      ]
    }
  }
]
```

## 📂 Structure des Fichiers

```text
src-tauri/src/rules_engine/
├── mod.rs          // Point d'entrée
├── ast.rs          // Définitions de l'Arbre Syntaxique (Enums Expr)
├── evaluator.rs    // Moteur d'exécution récursif
├── analyzer.rs     // Analyse statique des dépendances
├── store.rs        // Stockage et indexation des règles
└── README.md       // Documentation
```

## ⚠️ Sécurité

GenRules n'est **pas** un interpréteur JavaScript ou Lua.

- **Pas de boucles** : Impossible de créer des boucles infinies (sauf récursion de règles mal configurée, gérée par un compteur de passes max dans le `CollectionsManager`).
- **Pas d'I/O** : Le moteur ne peut pas lire de fichiers ou faire de requêtes réseau, sauf via le `DataProvider` strictement contrôlé (lecture DB locale uniquement).
