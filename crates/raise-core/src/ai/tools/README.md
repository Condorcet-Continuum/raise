# AI Tools (Moteur d'Exécution Symbolique)

Ce module contient les **outils MCP (Model Context Protocol)** utilisés par les agents intelligents (Software Agent, etc.) pour interagir avec le système de fichiers, la base de données et le moteur de génération de code.

C'est ici que s'opère le **"Grounding"** (l'ancrage) : l'IA passe de la génération de texte probabiliste à l'exécution d'actions déterministes et sécurisées.

## 📐 Architecture Globale

```mermaid
graph TD
    subgraph "Couche Agent (Neuro)"
        User((Utilisateur)) -->|Intent| Agent[Software Agent]
        Agent -->|Context| Tool[CodeGenTool]
    end

    subgraph "Couche Outils (Symbolique)"
        Tool -->|1. Fetch ID| DB[(JsonDB)]
        Tool -->|2. Config| GenService[CodeGeneratorService]
        GenService -->|3. Read Template| Templates
        GenService -->|4. Write| FS[Système de Fichiers]
    end

    subgraph "Domaine"
        DB -.->|Schema| Models[Modèles Arcadia]
        FS -.->|Artifacts| Code[Code Source Généré]
    end

    style Tool fill:#f9f,stroke:#333,stroke-width:2px
    style GenService fill:#bbf,stroke:#333,stroke-width:2px
```

## 🛠️ Outils Disponibles

### 1. `CodeGenTool` (`codegen_tool.rs`)

**Nom MCP :** `generate_component_code`

C'est l'outil principal pour l'ingénierie logicielle. Il fait le pont entre le modèle système et le code physique.

**Fonctionnalités Clés :**

- **Smart Linking :** Retrouve la configuration complète via UUID.
- **Multi-Langage :** Supporte Rust, C++, Python, VHDL, etc..
- **Round-Trip Engineering :** Préserve le code manuel utilisateur.

#### Flux d'Exécution

```mermaid
sequenceDiagram
    participant Agent
    participant Tool as CodeGenTool
    participant DB as JsonDB (Manager)
    participant Gen as CodeGenService
    participant FS as FileSystem

    Agent->>Tool: execute(component_id)
    activate Tool

    Tool->>DB: get_document(id)
    Note right of DB: Cherche dans pa_components,<br/>la_components, etc.
    DB-->>Tool: Component JSON

    Tool->>Tool: determine_language()

    Tool->>Gen: generate_for_element(json, lang)
    activate Gen

    Gen->>FS: Check existing file?
    alt Fichier existe
        FS-->>Gen: Contenu actuel
        Gen->>Gen: Extract Injections
    end

    Gen->>Gen: Render Templates + Inject Logic
    Gen->>FS: Write Files (Cargo.toml, lib.rs)

    Gen-->>Tool: List[Paths]
    deactivate Gen

    Tool-->>Agent: Success (Files list)
    deactivate Tool

```

---

### 2. `FileWriteTool` (`file_system.rs`)

**Nom MCP :** `fs_write`

Outil bas niveau permettant à un agent d'écrire ou de modifier des fichiers spécifiques.

**Sécurité :**

- **Sandbox :** L'outil est restreint à un `root_dir`.
- **Path Traversal :** Bloque les tentatives type `../secret.txt`.

## 🛡️ Sécurité & Round-Trip

### Protection du code manuel

Le système utilise des balises d'injection pour permettre la collaboration Homme-Machine. L'IA n'écrase jamais le code situé entre ces balises.

```mermaid
flowchart LR
    A[Nouveau Modèle] --> B(Générateur)
    C[Fichier Existant] --> D{Contient Code Manuel?}

    D -- Non --> B
    D -- Oui --> E[Extraction 'AI_INJECTION_POINT']
    E --> B

    B --> F[Fusion du Code]
    F --> G[Écriture Disque]

    style E fill:#9f9,stroke:#333
    style F fill:#9f9,stroke:#333

```

**Exemple de code protégé :**

```rust
pub fn analyser_flux_video() {
    // AI_INJECTION_POINT: analyser_flux_video
    // Le code écrit ici est IMMUABLE pour l'IA.
    opencv::process(...);
    // END_AI_INJECTION_POINT
}

```

## 🧪 Tests

Chaque outil dispose de tests unitaires et d'intégration robustes.

```bash
# Tester la génération complète (DB -> Tool -> Fichier)
cargo test ai::tools::codegen_tool

# Tester la sécurité du système de fichiers
cargo test ai::tools::file_system


```
