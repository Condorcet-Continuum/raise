# 🛠️ Module Tools (Native MCP)

Ce module implémente la couche d'**Interactions Physiques & Déterministes** du moteur Raise. Il permet à l'IA de passer du stade de "Penseur" (Brain) à celui d'"Acteur" (Hands) en interagissant avec le monde réel ou le système hôte.

## 🎯 Philosophie et Principes Directeurs

Contrairement aux _Agents_ qui sont probabilistes et conversationnels, les _Outils_ du moteur Raise doivent répondre à quatre impératifs stricts :

1. **Déterministes** : Pour une entrée donnée, l'outil doit produire une sortie prévisible et répétable.
2. **Atomiques** : Chaque outil possède une responsabilité unique pour faciliter la composition complexe dans le workflow.
3. **Typés et Auto-descriptifs** : Utilisation de schémas JSON pour la validation et de descriptions riches pour permettre au LLM de comprendre le contexte d'utilisation.
4. **Souverains et Sécurisés** : Exécutés nativement en Rust, ils garantissent que les données sensibles ne quittent jamais l'environnement local.

> **Architecture MCP (Model Context Protocol)** : Raise s'inspire du standard d'Anthropic mais l'implémente nativement pour éliminer la latence réseau et maximiser la performance système.

---

## 🏗️ Architecture Technique

### Le Trait `AgentTool`

Cœur du module, ce contrat définit comment le moteur communique avec le matériel ou les APIs système :

```rust
#[async_trait]
pub trait AgentTool: Send + Sync + Debug {
    fn name(&self) -> &str;           // Identifiant unique (ex: "read_system_metrics")
    fn description(&self) -> &str;    // Manuel d'utilisation pour le LLM
    fn parameters_schema(&self) -> Value; // Validation JSON Schema des entrées
    async fn execute(&self, args: &Value) -> Result<Value>; // Logique métier asynchrone
}

```

### Cycle de vie d'une exécution

1. **Déclenchement** : Un nœud `CallMcp` est atteint dans le graphe d'exécution.
2. **Validation** : Les arguments fournis sont validés par rapport au `parameters_schema`.
3. **Exécution** : L'implémentation Rust exécute l'action (lecture capteur, écriture fichier).
4. **Persistance** : Le résultat est injecté dans le contexte du workflow, le rendant disponible pour les nœuds suivants (ex: `GatePolicy`).

---

## 🚀 Guide de Développement : Créer un Outil

### 1. Définition de la logique (Exemple : `fs_tools.rs`)

Il est crucial de gérer les erreurs proprement via le type `Result` pour ne pas faire crash le moteur.

```rust
#[async_trait::async_trait]
impl AgentTool for FileReadTool {
    fn name(&self) -> &str { "read_file" }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Chemin absolu du fichier" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: &Value) -> Result<Value> {
        let path = args.get("path").and_then(|v| v.as_str()).ok_or("Path required")?;
        let content = fs::read_to_string(path).map_err(|e| format!("IO Error: {}", e))?;
        Ok(json!({ "content": content, "size": content.len() }))
    }
}

```

### 2. Enregistrement Système

L'outil doit être déclaré dans le `WorkflowScheduler` lors de son initialisation :

```rust
// Dans src-tauri/src/workflow_engine/scheduler.rs
executor.register_tool(Box::new(fs_tools::FileReadTool));

```

---

## 📦 Catalogue des Capacités Natives

| Outil                     | ID (`name`)           | Domaine       | Impact Sécurité         |
| ------------------------- | --------------------- | ------------- | ----------------------- |
| **Moniteur Système**      | `read_system_metrics` | Observabilité | Faible (Lecture seule)  |
| **Gestionnaire Fichiers** | `fs_write`            | Persistance   | Élevé (Écriture disque) |
| **Contrôleur Réseau**     | `network_ping`        | Connectivité  | Moyen                   |

---

## 🛡️ Sécurité et "Lignes Rouges" (Vetos)

L'intégration d'un outil dans un workflow est souvent couplée à un nœud `GatePolicy`. Cette architecture permet de créer des **Vetos automatiques** :

1. **Lecture** : `CallMcp` récupère une métrique (ex: `vibration_z`).
2. **Évaluation** : `GatePolicy` compare la valeur à un seuil critique défini dans le Mandat (ex: 8.0).
3. **Action** : Si le seuil est dépassé, le moteur interrompt immédiatement l'exécution avant que l'IA ne puisse agir.

```json
{
  "type": "call_mcp",
  "params": { "tool_name": "read_system_metrics", "arguments": { "sensor_id": "vibration_z" } }
}
```
