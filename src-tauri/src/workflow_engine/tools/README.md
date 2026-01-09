# 🛠️ Module Tools (Native MCP)

Ce module implémente la couche d'**Interactions Physiques & Déterministes** du moteur Raise.
Il permet à l'IA de passer du stade de "Penseur" (Brain) à celui d'"Acteur" (Hands).

## 🎯 Philosophie

Contrairement aux _Agents_ (`src/ai/agents`) qui sont probabilistes et conversationnels, les _Outils_ doivent être :

1. **Déterministes** : Pour une même entrée, toujours la même sortie.
2. **Atomiques** : Une seule responsabilité par outil.
3. **Typés** : Entrées et sorties structurées (JSON).
4. **Souverains** : Exécutés localement en Rust, sans dépendance cloud obscure.

> **Note :** Cette architecture s'inspire du standard **MCP (Model Context Protocol)** d'Anthropic, mais implémentée nativement en Rust pour des performances maximales et une latence nulle.

---

## 🏗️ Architecture

### Le Trait `AgentTool`

Tout outil doit implémenter ce contrat (interface) défini dans `mod.rs` :

```rust
#[async_trait]
pub trait AgentTool: Send + Sync + Debug {
    /// Nom unique pour l'appel (ex: "fs_write", "sensor_read")
    fn name(&self) -> &str;

    /// Description pour le LLM (Le "Mode d'Emploi")
    fn description(&self) -> &str;

    /// Schéma des paramètres attendus (JSON Schema)
    fn parameters_schema(&self) -> Value;

    /// L'action réelle
    async fn execute(&self, args: &Value) -> Result<Value>;
}

```

---

## 🚀 Comment créer un nouvel outil ?

Exemple : Créer un outil pour lire un fichier local.

### 1. Créer le fichier

Créez `src-tauri/src/workflow_engine/tools/fs_tools.rs`.

### 2. Implémenter le Trait

```rust
use super::AgentTool;
use crate::utils::Result;
use serde_json::{json, Value};
use std::fs;

#[derive(Debug)]
pub struct FileReadTool;

#[async_trait::async_trait]
impl AgentTool for FileReadTool {
    fn name(&self) -> &str { "read_file" }

    fn description(&self) -> &str {
        "Lit le contenu textuel d'un fichier sur le disque local."
    }

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
        let path = args.get("path").and_then(|v| v.as_str())
            .ok_or("Path required")?;

        let content = fs::read_to_string(path)
            .map_err(|e| format!("IO Error: {}", e))?;

        Ok(json!({ "content": content, "size": content.len() }))
    }
}

```

### 3. Enregistrer l'outil

Dans `src-tauri/src/workflow_engine/scheduler.rs` (méthode `new`) :

```rust
executor.register_tool(Box::new(fs_tools::FileReadTool));

```

---

## 📦 Catalogue d'Outils Actuels

| Outil                | ID (`name`)           | Description                                        | Paramètres                           |
| -------------------- | --------------------- | -------------------------------------------------- | ------------------------------------ |
| **Moniteur Système** | `read_system_metrics` | Lit CPU, RAM et capteurs simulés (Vibration/Temp). | `sensor_id`: "cpu", "vibration_z"... |

---

## 🔗 Intégration dans le Workflow

Les outils sont appelés via le nœud de type `CallMcp`.

**Exemple de définition JSON dans le Mandat :**

```json
{
  "id": "node_check_sensor",
  "type": "call_mcp",
  "name": "Vérification Capteur Z",
  "params": {
    "tool_name": "read_system_metrics",
    "arguments": {
      "sensor_id": "vibration_z"
    }
  }
}
```

Si l'outil renvoie une donnée critique (ex: vibration élevée), un nœud `GatePolicy` placé juste après peut déclencher un arrêt d'urgence.
