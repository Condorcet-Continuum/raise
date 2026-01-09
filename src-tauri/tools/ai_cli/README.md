# AI CLI — Interface de Commande Neuro-Symbolique

**Package :** `ai_cli`
**Localisation :** `src-tauri/tools/ai_cli`
**Rôle :** Outil de développement, de debug et d'automatisation pour le backend IA de RAISE.

---

## 🎯 Objectifs

L'`ai_cli` est un exécutable léger ("Thin Client") qui permet d'interagir directement avec la librairie `raise` sans passer par l'interface graphique (Tauri/React).

Il est essentiel pour :

1.  **Tester la "plomberie"** : Vérifier que le LLM Local (Mistral) ou Cloud (Gemini) répond bien.
2.  **Valider le NLU** : S'assurer que le `IntentClassifier` comprend bien les phrases de l'ingénieur.
3.  **Exécuter des Agents** : Lancer des tâches de modélisation sur **toutes les couches** (OA -> EPBS + Data + Transverse) directement depuis le terminal.

---

## 🏛️ Architecture & Flux

L'outil instancie les mêmes structures que le serveur Tauri (`LlmClient`, `StorageEngine`, `AgentContext`) mais dans un environnement CLI éphémère.

```text
┌──────────────┐
│  DEVELOPER   │
└──────┬───────┘
       │ cargo run -p ai_cli -- classify "..."
       ▼
┌──────────────────────┐        1. Identification         ┌───────────────────┐
│    AI_CLI BINARY     │ ───────────────────────────────▶ │ INTENT CLASSIFIER │
│ (src-tauri/tools...) │ ◀─────────────────────────────── │ (Rules + LLM)     │
└──────────┬───────────┘        2. EngineeringIntent      └───────────────────┘
           │
           │ 3. Dispatch (Business, System, Data, etc.)
           ▼
    ┌──────────────────────┐                                  ┌───────────────────┐
    │     AGENT SQUAD      │        4. Enrichissement         │    LLM BACKEND    │
    │ (OA/SA/LA/PA/EPBS..) │ ───────────────────────────────▶ │  (Local / Cloud)  │
    └──────────┬───────────┘ ◀──────────────────────────────── └───────────────────┘
               │                    5. JSON Content
               │
               │ 6. Persistance (Si flag -x)
               ▼
    ┌──────────────────────┐
    │   STORAGE ENGINE     │
    │ (Filesystem JSON-DB) │
    └──────────────────────┘

```

---

## ⚙️ Configuration

L'outil charge automatiquement le fichier `.env` situé à la racine du monorepo (`../../../../.env`).

**Variables Indispensables :**

```bash
# Choix du mode (Hybrid)
RAISE_MODE_DUAL="true"

# URLs & Clés
RAISE_LOCAL_URL="http://localhost:8080"
RAISE_GEMINI_KEY="AIza..."

# Cibles de données (Absolues de préférence)
PATH_RAISE_DOMAIN="/home/user/raise/data"
PATH_RAISE_DATASET="/home/user/raise/dataset"

```

---

## 🚀 Commandes Disponibles

### 1. `chat` (Mode Conversationnel)

Permet de tester la latence et la réponse brute du LLM (Mode RAG simulé).

**Syntaxe :**

```bash
cargo run -p ai_cli -- chat [OPTIONS] <MESSAGE>

```

**Options :**

- `-c, --cloud` : Force l'utilisation de Google Gemini (si configuré). Sinon, utilise LocalLlama.

**Exemple :**

```bash
cargo run -p ai_cli -- chat "Quelle est la différence entre OA et SA dans Arcadia ?"

```

---

### 2. `classify` (Mode Ingénierie)

C'est la commande principale. Elle simule le cycle complet : **Intention -> Agent -> DB**.

**Syntaxe :**

```bash
cargo run -p ai_cli -- classify [OPTIONS] <INPUT>

```

**Options :**

- `-x, --execute` : **Active l'écriture**. Sans ce flag, l'outil tourne en mode "Dry Run" (simulation) : il affiche l'intention détectée et l'agent qui _serait_ appelé, mais ne touche pas à la base de données.

**Scénarios supportés (Couverture Complète) :**

| Couche             | Intention Détectée           | Exemple de commande                           |
| ------------------ | ---------------------------- | --------------------------------------------- |
| **OA** (Business)  | `DefineBusinessUseCase`      | `"Je veux gérer les congés payés RH"`         |
| **SA** (Système)   | `CreateElement` (SA)         | `"Crée une fonction système Démarrer Moteur"` |
| **LA** (Logiciel)  | `CreateElement` (LA)         | `"Crée un composant AuthService"`             |
| **PA** (Matériel)  | `CreateElement` (PA)         | `"Crée un serveur Rack Dell R750"`            |
| **EPBS** (Config)  | `CreateElement` (EPBS)       | `"Ajoute un CI pour la carte mère"`           |
| **DATA** (Données) | `CreateElement` (DATA)       | `"Défini la classe Client avec nom et email"` |
| **TRANSVERSE**     | `CreateElement` (TRANSVERSE) | `"Ajoute une exigence de performance"`        |
| **CODE**           | `GenerateCode`               | `"Génère le code Rust pour une API REST"`     |

**Exemple Complet (Hardware) :**

```bash
# 1. Simulation (Dry Run)
cargo run -p ai_cli -- classify "Crée un FPGA Xilinx pour le traitement vidéo"

# Sortie :
# 🧠 Analyse de l'intention...
# 🔧 Intention Hardware détectée (PA) -> HardwareAgent
# (Mode Dry Run - Utilisez -x pour exécuter réellement)

# 2. Exécution Réelle
cargo run -p ai_cli -- classify "Crée un FPGA Xilinx pour le traitement vidéo" -x

# Sortie :
# 🧠 Analyse...
# 🔧 Exécution Hardware Agent (PA)...
# ✅ SUCCÈS :
# [Hardware] Electronics créé : VideoProcessingUnit (ID: ...)

```

---

## 🐛 Dépannage

| Erreur               | Solution                                                              |
| -------------------- | --------------------------------------------------------------------- |
| `Connection refused` | Vérifiez que votre serveur local (llama.cpp) tourne sur le port 8080. |
| `API Key Missing`    | Vérifiez votre fichier `.env`.                                        |
| `Partial move`       | Erreur de compilation Rust interne (signalez-le à l'équipe).          |
| `Schema not found`   | Le dossier `dataset` est mal configuré dans le `.env`.                |

---

> **Note Développeur :** Pour voir les logs détaillés (requêtes HTTP, parsing JSON), utilisez :
> `RUST_LOG=debug cargo run -p ai_cli ...`

```

```
