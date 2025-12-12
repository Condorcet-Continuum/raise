# Module `ai/llm` — Client d'Inférence Unifié (Dual Mode)

Ce module est la passerelle de communication bas niveau entre GenAptitude et les Large Language Models (LLMs).

Il implémente une architecture **Dual Mode** qui permet de basculer dynamiquement, requête par requête, entre une exécution souveraine (locale) et une exécution haute performance (cloud), le tout derrière une interface Rust unifiée.

## 🏗️ Architecture Technique

Le module agit comme un **Adaptateur Universel**. Le reste de l'application (Agents, Commandes) ne se soucie pas du format JSON spécifique de chaque fournisseur.

```mermaid
graph LR
    Caller[Agent / Command] -->|ask(backend, prompt)| Client[LlmClient]

    Client -->|Switch Backend| Router{Routeur}

    %% Branche Locale
    Router -- LocalLlama --> Adapter1[OpenAI Adapter]
    Adapter1 -->|POST /v1/chat/completions| Docker[🐳 Docker (Mistral 7B)]
    Docker --> GPU[Nvidia GTX/RTX]

    %% Branche Cloud
    Router -- GoogleGemini --> Adapter2[Google REST Adapter]
    Adapter2 -->|POST /v1beta/models/generateContent| Cloud[☁️ Google Vertex AI]

    %% Retour
    Docker -->|JSON| Parser[Response Unifier]
    Cloud -->|JSON| Parser

    Parser -->|String| Caller
```

---

## 📂 Composants

### 1\. `client.rs` (Le Driver)

C'est le cœur du module. Il encapsule toute la complexité réseau et protocolaire.

- **Gestionnaire HTTP** : Utilise `reqwest` avec des timeouts configurés (5 minutes) pour supporter les temps de génération longs des LLMs sur CPU/GPU local.
- **DTOs (Data Transfer Objects)** : Structures `Serialize`/`Deserialize` internes qui mappent les formats propriétaires :
  - `OpenAiRequest` : Format standard (llama.cpp, Ollama, vLLM).
  - `GeminiRequest` : Format spécifique Google (contents, parts, system_instruction).
- **Logique Unifiée** : La méthode `ask()` prend en charge le formatage du prompt système et utilisateur, l'envoi, et l'extraction propre du texte dans la réponse.

### 2\. `mod.rs`

Point d'entrée du module. Expose les structures publiques (`LlmClient`, `LlmBackend`) et contient les tests d'intégration.

---

## ⚙️ Configuration

Le client est "stateless" mais sa configuration est injectée au démarrage via les variables d'environnement (chargées par `dotenvy`).

| Variable                 | Rôle                                          | Exemple                 |
| :----------------------- | :-------------------------------------------- | :---------------------- |
| `GENAPTITUDE_LOCAL_URL`  | Adresse du serveur d'inférence local (Docker) | `http://localhost:8080` |
| `GENAPTITUDE_GEMINI_KEY` | Clé API Google AI Studio (Optionnel)          | `AIzaSy...`             |
| `GENAPTITUDE_MODEL_NAME` | Nom du modèle Cloud cible                     | `gemini-1.5-pro`        |

---

## 🚀 Guide d'Utilisation (Rust)

### Instanciation

Le client est conçu pour être instancié une fois au niveau de la commande ou du CLI, puis cloné (le clone est léger, c'est juste un pointeur vers le pool de connexions).

```rust
use crate::ai::llm::client::{LlmClient, LlmBackend};

// Configuration chargée depuis l'env
let client = LlmClient::new(
    "http://localhost:8080",
    "ma_cle_google",
    Some("gemini-1.5-pro".to_string())
);
```

### Appel Unifié (`ask`)

La méthode `ask` est asynchrone et retourne un `Result<String>`.

**Cas 1 : Rapidité & Confidentialité (Local)**
_Pour la classification d'intention, le chat simple, les petites corrections._

```rust
let response = client.ask(
    LlmBackend::LocalLlama,
    "Tu es un expert Rust.",      // System Prompt
    "Comment faire une struct ?"  // User Prompt
).await?;
```

**Cas 2 : Intelligence Complexe (Cloud)**
_Pour la génération de SML, l'analyse d'architecture, ou quand le GPU local sature._

```rust
let response = client.ask(
    LlmBackend::GoogleGemini,
    "Tu es un architecte système senior.",
    "Analyse les incohérences de ce modèle complexe..."
).await?;
```

---

## 🛡️ Sécurité et Robustesse

1.  **Isolation des Données** :

    - En mode `LocalLlama`, **aucune donnée ne quitte la machine**. Les paquets restent sur la boucle locale (`localhost`).
    - C'est le mode par défaut et privilégié pour les données sensibles.

2.  **Gestion d'Erreurs (Fail-fast)** :

    - Le client vérifie les statuts HTTP (`!res.status().is_success()`) avant de tenter de parser le JSON.
    - Les erreurs réseau (Docker éteint, Internet coupé) sont propagées via `anyhow` pour être affichées proprement à l'utilisateur.

3.  **Parsing Résilient** :

    - Utilisation de `Option<T>` pour les champs de réponse JSON qui peuvent manquer selon les versions d'API.

---

## 🔮 Roadmap Technique

- [ ] **Streaming (SSE)** : Implémenter `ask_stream()` pour recevoir la réponse token par token (effet "machine à écrire" dans l'UI).
- [ ] **Embeddings** : Ajouter une méthode `embed(text) -> Vec<f32>` pour vectoriser le texte (nécessaire pour le futur moteur de recherche sémantique).
- [ ] **Token Counting** : Estimer le nombre de tokens avant envoi pour éviter les erreurs "Context Length Exceeded".
- [ ] **Fallback Automatique** : Si le Cloud est inaccessible (timeout/erreur), basculer automatiquement sur le Local en mode dégradé.
