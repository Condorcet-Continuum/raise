# 🧮 Frontend Moteur de Règles (GenRules)

Ce répertoire contient l'interface utilisateur dédiée à la démonstration et à l'interaction avec le moteur de règles réactif **GenRules** (implémenté en Rust).

L'objectif de ces composants est de montrer comment **RAISE** peut fournir une validation "Correct-by-Construction" et des calculs en temps réel (champs dérivés) sans obliger l'utilisateur à sauvegarder ses données en base.

## 📂 Structure des Fichiers

```text
src/components/rules_engine/
├── RulesEngineDashboard.tsx  // Conteneur principal avec menu latéral
├── ModelRulesDemo.tsx        // Scénario Ingénierie (Validation & Naming)
├── InvoiceDemo.tsx           // Scénario Gestion (Calculs & Cross-Lookup)
└── README.md                 // Documentation (ce fichier)
```

---

## 🧩 Composants

### 1\. `RulesEngineDashboard.tsx`

C'est le point d'entrée du module. Il agit comme un "Layout" local.

- **Rôle** : Gère la navigation entre les différents scénarios de démonstration.
- **Structure** : Utilise un **Menu Latéral** à gauche pour sélectionner le contexte (`activeTab`) et affiche le composant correspondant à droite.
- **Navigation** : Bascule entre `'model'` (Ingénierie) et `'invoice'` (Facturation).

### 2\. `ModelRulesDemo.tsx` (Scénario Ingénierie)

Démontre l'application des règles dans un contexte de modélisation système (type Arcadia/Capella).

- **Cas d'usage** : Création d'une `LogicalFunction`.
- **Règles testées** :
  - **Conformité (Regex)** : Le nom doit commencer par `LF_` et être en majuscules.
  - **Champ Calculé** : `full_path` est concaténé automatiquement (`Package::Name`).
- **Interaction Backend** :
  - Bouton **"Reset Rules"** : Appelle `jsondb_init_model_rules` pour générer le schéma JSON sur le disque (`v1/la/functions.json`).
  - Feedback visuel : Badges Vert/Rouge selon le statut de conformité retourné par le moteur.

### 3\. `InvoiceDemo.tsx` (Scénario Gestion)

Démontre les capacités de calcul arithmétique et de liaison de données (Lookup).

- **Cas d'usage** : Création d'une Facture.
- **Règles testées** :
  - **Cross-Collection Lookup** : Récupère le TJM d'un utilisateur depuis la collection `users` via son ID (`u_dev`).
  - **Maths** : Calcule le total (`days * tjm`).
  - **Dates** : Calcule l'échéance (`created_at + 30 jours`).
- **Interaction Backend** :
  - Bouton **"Setup Démo"** : Appelle `jsondb_init_demo_rules` pour créer les collections `users` et `invoices` avec leurs données initiales.

---

## 🔄 Flux de Données (Architecture)

Ces composants ne calculent rien eux-mêmes en TypeScript. Ils délèguent toute la logique au Backend Rust pour garantir que les règles appliquées dans l'UI sont **exactement les mêmes** que celles appliquées lors de la persistance en base de données.

1.  **Saisie** : L'utilisateur tape dans un champ (ex: `days`).
2.  **Hook** : Le hook `useRulesEngine` (situé dans `src/hooks/`) détecte le changement.
3.  **Debounce** : Après une courte pause (500ms), une requête est envoyée.
4.  **Tauri** : La commande `jsondb_evaluate_draft` est invoquée avec le document JSON courant.
5.  **Rust** : Le moteur charge le schéma, exécute l'AST (Arbre Syntaxique Abstrait) des règles, et enrichit le JSON.
6.  **Mise à jour** : Le Frontend reçoit le nouveau JSON et met à jour l'état React.

## 🛠️ Utilisation

Pour utiliser ces démos, assurez-vous que le backend Rust est en cours d'exécution.

1.  Accédez à la section **Moteur de Règles** depuis le Dashboard principal ou le menu latéral.
2.  Cliquez sur le bouton d'initialisation (ex: "🛠️ Setup Démo") la première fois. Cela est nécessaire pour écrire les fichiers `.schema.json` physiques que le backend doit lire.
3.  Modifiez les champs et observez les mises à jour automatiques.
