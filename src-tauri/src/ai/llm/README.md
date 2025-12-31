# Module `ai::llm` - Infrastructure Bas Niveau LLM

Ce module constitue la couche d'infrastructure (**Low-Level Layer**) de GenAptitude pour la communication avec les modèles de langage. Il fournit la "tuyauterie" technique permettant aux Agents de fonctionner sans se soucier de la complexité réseau ou du formatage des réponses.

---

## 📂 Structure du Module

Voici l'organisation physique des fichiers de ce module :

```text
src-tauri/src/ai/llm/
├── mod.rs               # Point d'entrée : expose les sous-modules publics.
├── client.rs            # Client HTTP : gère la connexion (Ollama/Gemini) et le Fallback.
├── prompts.rs           # Personas : contient les constantes des "System Prompts".
├── response_parser.rs   # Nettoyeur : extrait le JSON/Code des réponses brutes.
└── tests.rs             # Validation : tests unitaires et d'intégration.

```

---

## 📊 Architecture & Flux de Données

Le système implémente une stratégie **"Local First"** avec un mécanisme de **Nettoyage Automatique** des réponses.

### Schéma du Flux (Pipeline)

```text
    +-----------+                                     +-----------------+
    |   AGENT   |  >> 1. Envoi du Prompt (Persona) >> |   LLM CLIENT    |
    +-----------+                                     +-----------------+
          ^                                                    |
          |                                          (Tentative Local : OLLAMA)
          |                                                    v
    (Retour JSON)                                    [ ECHEC ? -> FALLBACK ]
          |                                                    |
          |                                           (Tentative Cloud : GEMINI)
          |                                                    |
    +-----------+                                              |
    |   PARSER  |  << 3. Nettoyage (No Markdown) <<   (Réponse Brute)
    +-----------+

```

### Description des Étapes

1. **Conditionnement (`prompts.rs`) :** L'Agent sélectionne une personnalité (ex: `SYSTEM_AGENT_PROMPT`) pour orienter l'expertise du modèle.
2. **Transport & Résilience (`client.rs`) :**

- Le client tente d'abord d'interroger le modèle local (port 11434 ou 8080).
- Si le serveur local ne répond pas, il bascule automatiquement sur l'API Google Gemini (si la clé est configurée).

3. **Nettoyage (`response_parser.rs`) :**

- La réponse brute arrive souvent polluée (ex: "Voici le JSON : `json ... `").
- Le parser extrait chirurgicalement les données utiles (JSON ou Code) avant de les renvoyer à l'Agent.

---

## 💻 Exemples d'Utilisation (Rust)

Voici comment utiliser les briques de ce module pour construire un Agent.

### Cas 1 : Analyse d'Intention (Retour JSON)

Ce cas est utilisé par le `IntentClassifier` pour router la demande.

````rust
use crate::ai::llm::{client, prompts, response_parser};

async fn classify_user_request(user_input: &str) -> Result<serde_json::Value, String> {
    // 1. Initialisation du Client (souvent fait au démarrage de l'app)
    // On cible le port par défaut d'Ollama
    let llm_client = client::LlmClient::new("http://localhost:11434", "optional_api_key", None);

    // 2. Construction du Prompt avec le Persona "Routeur"
    let full_prompt = format!(
        "{}\n\nUSER REQUEST: {}",
        prompts::INTENT_CLASSIFIER_PROMPT,
        user_input
    );

    // 3. Envoi de la requête (Le client gère le réseau et le fallback)
    let raw_response = llm_client.ask_raw(&full_prompt).await
        .map_err(|e| format!("Erreur LLM: {}", e))?;

    // 4. Nettoyage et Parsing JSON
    // Cela gère les cas où l'IA répond "Voici le JSON : ```json { ... } ```"
    let json_data = response_parser::extract_json(&raw_response)
        .map_err(|e| format!("Erreur Parsing: {}", e))?;

    // On retourne l'objet JSON propre
    Ok(json_data)
}

````

### Cas 2 : Génération de Code (Retour Texte Brut)

Ce cas est utilisé par le `SoftwareAgent` pour écrire des fichiers Rust.

````rust
use crate::ai::llm::{client, prompts, response_parser};

async fn generate_rust_code(task_description: &str) -> Result<String, String> {
    let llm_client = client::LlmClient::new("http://localhost:11434", "", None);

    // On utilise le Persona "Software Engineer"
    let prompt = format!("{}\nTask: {}", prompts::SOFTWARE_AGENT_PROMPT, task_description);

    let raw_response = llm_client.ask_raw(&prompt).await
        .map_err(|e| e.to_string())?;

    // Ici, on ne veut pas parser du JSON, mais extraire le bloc de code
    // Cette fonction retire le texte "Voici le code" et les balises ```rust
    let clean_code = response_parser::extract_code_block(&raw_response);

    Ok(clean_code)
}

````

---

## ⚙️ Configuration Requise

Variables d'environnement (fichier `.env` ou contexte d'exécution) :

| Variable                    | Description                                                 |
| --------------------------- | ----------------------------------------------------------- |
| `GENAPTITUDE_LLM_LOCAL_URL` | URL du serveur local (défaut : `http://localhost:11434/v1`) |
| `RAISE_GEMINI_KEY`          | Clé API de secours (Google AI Studio)                       |

---

## ✅ Validation

Pour vérifier que ce module fonctionne correctement (Parser + Prompts + Client), exécutez la suite de tests dédiée :

```bash
cargo test ai::llm

```
