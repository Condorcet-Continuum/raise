Based on the provided test suite configuration files (`json_db_suite.rs`, `rules_suite.rs`, `code_gen_suite.rs`, `ai_suite.rs`), here is the documentation for the `src-tauri/tests/` directory. This directory orchestrates the validation of all major components of the RAISE backend.

---

# Tests d'Intégration (Rust)

Ce répertoire contient l'ensemble des suites de tests d'intégration pour le backend Rust de RAISE. Ces tests valident le fonctionnement conjoint des différents modules (Stockage, Sémantique, IA, Règles) dans des environnements isolés et reproductibles.

## 📂 Organisation des Suites

L'architecture de tests est découpée par domaine fonctionnel majeur, chaque fichier racine (`*_suite.rs`) orchestrant plusieurs sous-modules de test.

### 1\. Suite Base de Données (`json_db_suite.rs`)

Valide le moteur de base de données embarqué JSON-DB. C'est la suite la plus critique pour l'intégrité des données.

- **Responsabilité** : Cycle de vie des données, transactions ACID, requêtes SQL, validation de schémas.
- **Modules** :
  - `lifecycle` : Création/Suppression de DB et Collections.
  - `transactions` : Atomicité (Commit/Rollback), WAL.
  - `query` : Parsing et exécution SQL, Indexation.
  - `schema` : Validation structurelle et cohérence des schémas embarqués.
- **Helper** : `init_test_env()` crée un environnement temporaire (`tempfile`) avec une copie réelle des schémas JSON (`schemas/v1`) pour garantir des tests réalistes sans polluer le disque.

### 2\. Suite Moteur de Règles (`rules_suite.rs`)

Valide le moteur réactif **GenRules**.

- **Responsabilité** : Calculs dynamiques, propagation des changements, logique métier déclarative.
- **Modules** :
  - `logic_scenarios` : Tests unitaires des opérateurs (Maths, Logique) avec mock.
  - `rules_integration` : Scénarios "End-to-End" (ex: Facturation avec Lookup cross-collection).

### 3\. Suite Intelligence Artificielle (`ai_suite.rs`)

Valide l'intégration des modèles de langage (LLM) et des agents autonomes.

- **Responsabilité** : Connectivité LLM, classification d'intention, orchestration d'agents.
- **Modules** :
  - `llm_tests` : Vérification de la configuration (Cloud vs Local) et connectivité basique (Ping).
  - `agent_tests` : Scénarios complexes où un agent reçoit une instruction en langage naturel et effectue une action concrète (ex: créer un acteur).

### 4\. Suite Génération de Code (`code_gen_suite.rs`)

Valide le générateur de code "Neuro-Symbolique".

- **Responsabilité** : Transformation des modèles sémantiques en code source (Rust, Python, etc.).
- **Modules** :
  - `rust_tests` : Génération de squelettes de code Rust valides à partir de définitions d'acteurs.
  - `agent_tests` : Intégration avec l'IA pour la génération assistée.

---

## 🚀 Guide d'Exécution

Rust permet d'exécuter les tests par suite ou globalement.

### Lancer tous les tests (Long)

```bash
cargo test
```

### Lancer une suite spécifique (Recommandé)

Pour iterer rapidement sur un module, lancez uniquement sa suite :

```bash
# Tester uniquement la base de données
cargo test --test json_db_suite

# Tester le moteur de règles
cargo test --test rules_suite
```

### Voir les logs (Debug)

Par défaut, Rust capture la sortie standard. Pour voir les `println!` et les logs :

```bash
cargo test --test rules_suite -- --nocapture
```

## 🛠️ Architecture Technique des Tests

- **Isolation** : Chaque test utilise `tempfile::tempdir` pour créer un dossier de données unique qui est automatiquement supprimé à la fin du test. Cela garantit qu'aucun état ne persiste entre deux exécutions.
- **Fixtures** : Les tests s'appuient sur une copie réelle du dossier `schemas/v1` du projet, garantissant que le code est testé contre les vrais modèles de données de production.
- **Déclarations Explicites** : Les fichiers `*_suite.rs` utilisent la directive `#[path = "..."]` pour mapper explicitement les sous-modules de test situés dans des dossiers éponymes, gardant la racine `tests/` propre.
