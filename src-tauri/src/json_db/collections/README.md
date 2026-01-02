# Module Collections (JSON-DB)

Ce module constitue la **façade de haut niveau** pour la manipulation des documents dans la base de données JSON de RAISE. Il orchestre le stockage, la validation, l'indexation et l'exécution des règles métier.

## 🏗️ Architecture

Le module est divisé en trois couches distinctes :

1.  **Façade (`mod.rs`)** : L'API publique exposée au reste de l'application. Elle redirige les appels vers le _Manager_ ou la couche _Collection_ selon le besoin.
2.  **Orchestrateur (`manager.rs`)** : Le "cerveau" du module. Il gère le cycle de vie complet d'un document (préparation, règles métier `GenRules`, validation de schéma, sémantique JSON-LD).
3.  **Stockage (`collection.rs`)** : La couche "I/O" brute. Elle gère uniquement la lecture et l'écriture atomique sur le disque, sans logique métier.

## 🚀 Pipeline d'Insertion / Mise à jour

Lorsqu'un document est inséré via `insert_with_schema`, il traverse le pipeline suivant (défini dans `manager.rs`):

1.  **Injection Automatique** :
    - Génération d'un UUID v4 si le champ `id` est manquant.
    - Injection des dates `createdAt` et `updatedAt` (ISO 8601).
    - Injection de l'URI du schéma dans `$schema` si disponible.
2.  **Moteur de Règles (GenRules)**:
    - Chargement des règles déclaratives `x_rules` depuis le schéma JSON.
    - Calcul des dépendances et exécution réactive (point fixe).
    - _Note:_ Les règles peuvent faire des "Lookup" vers d'autres collections via le `DataProvider`.
3.  **Validation JSON Schema** :
    - Vérification stricte de la structure et des types via `validator_cli`.
4.  **Enrichissement Sémantique** :
    - Injection du contexte JSON-LD (`@context`).
    - Vérification des types ontologiques (`oa:`, `sa:`, etc.).
5.  **Persistance** :
    - Écriture atomique du fichier JSON sur le disque.
6.  **Indexation** :
    - Mise à jour de l'index système `_system.json`.
    - Mise à jour des index secondaires via `IndexManager`.

## 🛠️ API Publique (`mod.rs`)

### Manipulation de Collections

- **`create_collection`** : Crée le dossier et le fichier `_meta.json`.
- **`drop_collection`** : Supprime physiquement le dossier et nettoie l'index système.

### Manipulation de Documents

- **`insert_with_schema`** : La méthode recommandée. Applique tout le pipeline (Règles + Validation + Write).
- **`insert_raw`** : Insertion bas niveau (déconseillé pour les données métier), contourne les règles mais maintient l'index système.
- **`update_with_schema`** : Similaire à l'insertion, recalcule les règles et met à jour `updatedAt`.
- **`get`** : Récupère un document par son ID.
- **`delete`** : Supprime un document et nettoie les index.
- **`list_all`** / **`list_ids`** : Utilitaires pour parcourir une collection.

## 🧠 Moteur de Règles (GenRules)

Le `CollectionsManager` intègre le moteur de règles réactif.

- **Source** : Les règles sont définies dans la propriété `x_rules` des schémas JSON.
- **Exécution** : `manager::apply_business_rules`.
- **Capacités** :
  - Mathématiques, Dates, Chaînes de caractères, Logique booléenne.
  - **Cross-Collection Lookup** : Capacité de lire des valeurs dans d'autres collections (ex: lire le TJM d'un utilisateur pour calculer une facture).
  - **Récursivité** : Le moteur détecte les changements profonds (`foo.bar.baz`) et propage les mises à jour jusqu'à stabilité.

## 📂 Structure de Fichiers

```text
src-tauri/src/json_db/collections/
├── mod.rs          // Point d'entrée et exports publics
├── manager.rs      // Logique métier, règles, validation, cycle de vie
└── collection.rs   // Opérations système de fichiers (FS)
```

## ⚠️ Notes Importantes

- **Index Système** : Le fichier `_system.json` à la racine de la DB est critique. Il est maintenu automatiquement par le `CollectionsManager`.
- **Atomicité** : Les écritures utilisent `atomic_write` pour éviter la corruption de fichiers en cas de crash.
