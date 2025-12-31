# 🤖 Suite de Tests d'Intégration IA (`ai_suite`)

Ce dossier contient les tests "End-to-End" (E2E) validant la chaîne complète de l'Intelligence Artificielle de GenAptitude. Ces tests ne simulent pas seulement la logique, ils vérifient que les **Agents** produisent réellement des fichiers JSON valides sur le disque, conformes au métamodèle Arcadia et résistants aux aléas des LLMs.

## 🏗️ Architecture du Test E2E

Chaque test instancie un environnement isolé (sandbox) et simule le comportement d'un ingénieur demandant une action à l'IA.

```text
┌────────────────┐      1. Init      ┌─────────────────────┐
│   TEST RUNNER  │ ────────────────▶ │  AiTestEnv (Setup)  │
└───────┬────────┘                   │ - Temp Dir (/tmp/x) │
        │                            │ - Storage Engine    │
        │ 2. Intent                  │ - LLM Client        │
        ▼                            └──────────┬──────────┘
┌────────────────┐                              │
│  AGENT (SUT*)  │ ◀────────────────────────────┘
└───────┬────────┘
        │ 3. Prompt (Context + Schema)
        ▼
┌────────────────┐      4. JSON      ┌─────────────────────┐
│  LLM BACKEND   │ ────────────────▶ │    JSON DATABASE    │
│ (Local/Cloud)  │  (Nettoyage Auto) │   (StorageEngine)   │
└────────────────┘                   └──────────┬──────────┘
                                                │
                                                │ 5. Write .json
        6. Assertion (Robustesse & Contenu)     ▼
┌────────────────┐                   ┌─────────────────────┐
│   VERIFICATION │ ◀──────────────── │  FILESYSTEM (Disk)  │
└────────────────┘                   └─────────────────────┘
```

\*_SUT : System Under Test_

---

## 🛡️ Robustesse & Validation

Ces tests valident spécifiquement la capacité du backend à gérer les **"Small Language Models" (SLM)** locaux (Mistral, Llama 3) qui peuvent être instables.

- **Extraction Chirurgicale** : On vérifie que l'agent ignore le texte "bavard" (Markdown, Intro, Outro) autour du JSON.
- **Protection des Données** : On vérifie que l'agent force le respect des consignes critiques (ex: le `name` du fichier EPBS doit correspondre à la demande, même si le LLM le renomme).
- **Tolérance Structurelle** : Les assertions acceptent des variations mineures (ex: liste d'attributs vide si le modèle est "paresseux").

---

## 📂 Catalogue des Scénarios de Test

Les tests sont organisés pour couvrir chaque couche du cycle en V et les aspects transverses.

### Suite Principale (`ai_suite`)

| Couche                | Fichier Test                | Objectif du Scénario                                                            |
| --------------------- | --------------------------- | ------------------------------------------------------------------------------- |
| **OA** (Métier)       | `business_agent_tests.rs`   | Analyse d'un besoin flou -> Création de **Capabilities** et **Actors**.         |
| **SA** (Système)      | `system_agent_tests.rs`     | "Le système doit..." -> Création de **SystemFunctions**.                        |
| **LA** (Logiciel)     | `software_agent_tests.rs`   | Architecture logique -> Création de **Components**.                             |
| **PA** (Matériel)     | `hardware_agent_tests.rs`   | Distinction Auto -> **FPGA** (Electronics) vs **Server** (Infrastructure).      |
| **EPBS** (Config)     | `epbs_agent_tests.rs`       | Industrialisation -> Création de **ConfigurationItems** (P/N généré).           |
| **DATA** (MDM)        | `data_agent_tests.rs`       | Dictionnaire -> Création de **Classes** et **Enums** (Nettoyage JSON agressif). |
| **IVVQ** (Transverse) | `transverse_agent_tests.rs` | Cycle Qualité -> **Exigence** -> **TestProcedure** -> **Campagne**.             |
| **INFRA**             | `llm_tests.rs`              | Vérifie que le serveur LLM (Ollama/Llama) répond (Ping).                        |

### Suite Code (`code_gen_suite`)

| Couche   | Fichier Test     | Objectif du Scénario                                            |
| -------- | ---------------- | --------------------------------------------------------------- |
| **CODE** | `agent_tests.rs` | Génération de code source (Rust/Python) avec contexte tolérant. |

---

## 🚀 Exécuter les Tests

Ces tests nécessitent un Backend LLM actif (Localhost:8080 ou Clé API). Ils sont marqués `#[ignore]` pour ne pas bloquer la CI par défaut.

### 1. Lancer toute la suite (Validation Complète)

```bash
# Suite principale (Agents de modélisation)
cargo test --test ai_suite -- --ignored

# Suite de génération de code
cargo test --test code_gen_suite -- --ignored

```

### 2. Tester un Agent spécifique (Debug Mode)

Utilisez l'option `--nocapture` pour voir les logs `[DEBUG LLM RAW]` et comprendre ce que le LLM renvoie réellement.

**Exemple : Debug Data Agent (Parsing JSON)**

```bash
cargo test --test ai_suite data_agent_tests -- --ignored --nocapture

```

**Exemple : Debug EPBS Agent (Configuration)**

```bash
cargo test --test ai_suite epbs_agent_tests -- --ignored --nocapture

```

---

## ⚙️ Configuration (`mod.rs`)

Le fichier `mod.rs` contient la logic de **Setup/Teardown**.

- **`init_ai_test_env()`** :
- Charge les variables `.env`.
- Crée un dossier temporaire unique (ex: `/tmp/.tmpXyZ`).
- Initialise un `StorageEngine` pointant vers ce dossier.
- Configure le `LlmClient` (Priorité : Local > Cloud).

---

## ⚠️ Dépannage Fréquent

**Erreur : `SKIPPED: Pas d'IA disponible**`

> Le test a détecté qu'aucune clé API n'est présente et que `http://localhost:8080/health` ne répond pas. Lancez votre serveur Ollama ou configurez `RAISE_GEMINI_KEY`.

**Erreur : `panicked at ... byte index ... is out of bounds**`

> (Obsolète) Ce crash indiquait un parsing JSON fragile. Il a été corrigé par l'introduction de la méthode `extract_json` sécurisée dans tous les agents. Si cela se reproduit, vérifiez `TransverseAgent` ou `DataAgent`.

**Erreur : `Assertion failed: found**`

> L'agent a fonctionné, mais le contenu du fichier ne contient pas les mots-clés attendus.
>
> - Vérifiez les logs avec `--nocapture`.
> - Le LLM a peut-être reformulé le nom (ex: "Server" au lieu de "Rack Server").

```

```

```

```
