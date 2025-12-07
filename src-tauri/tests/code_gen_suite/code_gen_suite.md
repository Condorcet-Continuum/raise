# 🧪 Suite de Tests d'Intégration : Code Gen & IA

Ce module de test (`code_gen_suite`) valide la chaîne de valeur complète de l'ingénierie augmentée par l'IA : de l'intention en langage naturel jusqu'à la persistance en base de données et la génération de code source.

---

## 🎯 Objectifs

Cette suite ne teste pas des fonctions isolées, mais des **flux complets** (End-to-End) :

1.  **Connectivité IA** : Vérifie que le client LLM (Gemini ou Local) répond correctement.
2.  **Agents Autonomes** : Valide que le `SystemAgent` comprend une intention et agit sur la base de données.
3.  **Intégrité JSON-DB** : S'assure que les données générées par l'IA respectent les schémas stricts.
4.  **Génération de Code** : Vérifie que le `CodeGeneratorService` produit des fichiers Rust valides à partir des modèles.

---

## ⚙️ Environnement de Test (`AiTestEnv`)

Pour garantir l'isolation et la reproductibilité, chaque test instancie un environnement `AiTestEnv` (défini dans `mod.rs`).

### Caractéristiques du Mock

- **Stockage Temporaire** : Utilise `tempfile` pour créer un dossier jetable.
- **Bootstrap DB Complet** : Simule l'arborescence de production GenAptitude :
  - Espace : `un2` (Convention Arcadia).
  - Base : `_system`.
  - Schémas : Crée physiquement des fichiers de schéma valides dans `schemas/v1/arcadia/oa/` (ex: `actor.schema.json`).
  - Index : Génère un `_system.json` valide pointant vers ces schémas.
- **Client LLM** : Initialisé avec les variables d'environnement du système.

### Structure Physique Simulée

```text
/tmp/test_dir_xyz/
├── un2/
│   ├── _system/
│   │   ├── _system.json  <-- Index critique
│   │   ├── schemas/v1/
│   │   │   └── arcadia/oa/actor.schema.json
│   │   └── collections/
│   │       └── actors/   <-- Là où l'agent écrit
```

---

## 🚀 Exécution des Tests

Certains tests nécessitent une infrastructure externe (Docker/Ollama) et sont marqués `#[ignore]` par défaut.

### 1\. Tests Unitaires (Rapides)

Testent la logique interne sans appel réseau lourd.

```bash
cargo test --test code_gen_suite
```

### 2\. Tests d'Intégration (Lents / Externes)

Ces tests requièrent un LLM local actif (sur `localhost:8080` ou compatible OpenAI).

```bash
cargo test --test code_gen_suite -- --ignored
```

---

## 🧪 Scénarios de Test

### `test_local_llm_connectivity`

- **But** : Ping le serveur d'inférence local.
- **Action** : Envoie "Réponds PONG".
- **Validation** : Reçoit une réponse non vide.

### `test_intent_classification_integration`

- **But** : Vérifie le classifieur d'intentions (NLU).
- **Input** : _"Crée une fonction système nommée 'Démarrer Moteur'"_
- **Validation** : Vérifie que l'intention détectée est `CreateElement` avec `layer: SA` et `type: Function`.

### `test_system_agent_creates_actor_end_to_end` (Critique)

C'est le test le plus complet.

1.  **Setup** : Initialise `AiTestEnv` (DB vide).
2.  **Action** : Demande à l'agent : _"Crée un acteur opérationnel nommé 'TestUnitBot'"_.
3.  **Processus** :
    - L'agent analyse l'intention.
    - Il génère le JSON (avec description via LLM).
    - Il appelle `CollectionsManager` pour insérer.
    - `CollectionsManager` valide le schéma, injecte l'ID et le contexte JSON-LD.
4.  **Validation** : Le test va lire physiquement le disque dans le dossier temporaire pour vérifier que le fichier JSON existe et contient les données.

### `test_rust_skeleton_generation`

- **But** : Valide le moteur de templates.
- **Action** : Fournit un objet JSON `OperationalActor`.
- **Validation** : Vérifie qu'un fichier `.rs` est créé avec la bonne structure (`struct`, `impl`).

---

## ⚠️ Dépannage

**Erreur : "Index \_system.json introuvable"**

> Le bootstrap de l'environnement de test a échoué. Vérifiez `mod.rs` et assurez-vous que `create_dir_all` a les droits d'écriture dans `/tmp`.

**Erreur : "Schema not found in registry"**

> L'agent cherche le schéma à un endroit (ex: `arcadia/oa/...`) mais le test l'a créé ailleurs (ex: `actors/...`). L'arborescence dans `mod.rs` doit correspondre exactement aux attentes du `SystemAgent`.

**Erreur : "Le dossier 'actors' doit avoir été créé"**

> L'agent a écrit ses données dans un espace (`space`) différent de celui surveillé par le test. Vérifiez que `AiTestEnv` utilise bien `un2` comme espace par défaut.
