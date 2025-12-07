# 🤖 Suite de Tests IA & Agents (`ai_suite`)

Ce module de test valide la couche d'intelligence artificielle de GenAptitude. Il s'assure que le système peut communiquer avec les LLMs (Locaux ou Cloud) et que les Agents autonomes (ex: `SystemAgent`) se comportent comme prévu.

---

## 🎯 Objectifs

Cette suite couvre deux aspects critiques :

1.  **Connectivité (LLM Client)** : Vérifie que la plomberie technique (HTTP, Auth, Timeouts) vers les modèles d'IA fonctionne.
2.  **Comportement Agentique (Behavior)** : Vérifie qu'un Agent comprend une intention (NLU) et effectue les actions concrètes sur le système de fichiers (JSON-DB).

---

## ⚙️ Environnement de Test (`AiTestEnv`)

Défini dans `mod.rs`, cet environnement garantit l'isolation des tests.

- **Stockage Temporaire** : Utilise `tempfile` pour créer une base de données JSON jetable.
- **Configuration Hybride** :
  - Charge les variables d'environnement (`.env`) pour les clés API.
  - Configure un `StorageEngine` pointant vers le dossier temporaire.
- **Client LLM** : Pré-configuré avec l'URL locale (`localhost:8080`) et la clé Gemini.

---

## 🚀 Exécution des Tests

La suite distingue les tests de configuration (rapides) des tests d'inférence (lents/externes).

### 1\. Tests de Configuration (Rapides)

Vérifient uniquement que les clés API sont présentes et que les structures s'instancient.

```bash
cargo test --test ai_suite
```

### 2\. Tests d'Intégration (Nécessite LLM Local)

Ces tests effectuent de vrais appels réseaux vers le LLM. Ils sont marqués `#[ignore]` pour ne pas bloquer la CI/CD standard.

**Prérequis :** Un serveur d'inférence (Llama.cpp / Ollama) doit tourner sur le port 8080.

```bash
cargo test --test ai_suite -- --ignored
```

---

## 🧪 Scénarios de Test

### 1\. Connectivité (`llm_tests.rs`)

- **`test_cloud_llm_config`** :
  - Vérifie simplement la présence et la longueur de la clé API Gemini.
  - _Ne fait pas d'appel réseau._
- **`test_local_llm_connectivity`** (Ignored) :
  - Effectue un "Ping" sémantique.
  - Prompt : _"Tu es un test unitaire. Réponds juste 'PONG'."_
  - Validation : La réponse ne doit pas être vide.

### 2\. Agents & NLU (`agent_tests.rs`)

- **`test_intent_classification_integration`** (Ignored) :
  - Valide le `IntentClassifier`.
  - Input : _"Crée une fonction système nommée 'Démarrer Moteur'"_.
  - Validation : Vérifie que l'intention retournée est bien `CreateElement`, couche `SA`, type `Function`.
- **`test_system_agent_creates_actor_end_to_end`** (Critique) :
  - Teste la chaîne complète : **Intention -\> Agent -\> DB**.
  - Action : L'agent reçoit l'ordre de créer un Acteur.
  - Vérification : Le test va scanner physiquement le dossier temporaire `un2/_system/collections/actors` pour vérifier :
    1.  La présence du fichier JSON.
    2.  Que le contenu inclut une **description générée par l'IA** (preuve que le LLM a travaillé).

---

## ⚠️ Dépannage

**`SKIPPED: Serveur local introuvable`**

> Le client n'a pas réussi à joindre `http://localhost:8080/health`. Vérifiez que votre conteneur Docker ou votre serveur Ollama est lancé.

**`Assertion failed: found` (dans `agent_tests`)**

> L'agent a bien tourné, mais le fichier n'a pas été trouvé sur le disque.
>
> - Vérifiez que l'agent écrit bien dans l'espace `un2` (défaut).
> - Vérifiez les logs pour voir si une erreur de validation JSON-Schema a empêché l'écriture.

**`Panic: Classification échouée`**

> Le LLM a "halluciné" et n'a pas respecté le format de sortie JSON strict demandé par le `IntentClassifier`. Relancez le test (le LLM est non-déterministe) ou ajustez le System Prompt.
