# 🛠️ Module Tools (Native MCP & Data-Driven)

Ce module implémente la couche d'**Interactions Physiques & Déterministes** du moteur Raise. Il permet à l'IA de passer du stade de "Penseur" (Brain) à celui d'"Acteur" (Hands) en interagissant avec le monde réel ou le Jumeau Numérique.

## 🎯 Philosophie et Principes Directeurs

Contrairement aux _Agents_ qui sont probabilistes et conversationnels, les _Outils_ du moteur Raise doivent répondre à des impératifs stricts :

1. **Déterministes** : Pour une entrée donnée, l'outil doit produire une sortie prévisible et répétable.
2. **Stateless & Connectés au Graphe** : Les outils n'instancient plus de connexions lourdes. Ils reçoivent un `HandlerContext` contenant le `CollectionsManager`, leur donnant un accès asynchrone et ultra-rapide à la JSON-DB.
3. **Typés et Auto-descriptifs** : Utilisation de schémas JSON pour la validation et de descriptions riches pour le routage de l'Agent.
4. **Souverains et Sécurisés** : Exécutés nativement en Rust, ils garantissent que les données sensibles ne quittent jamais l'environnement local.

> **Architecture MCP (Model Context Protocol)** : Raise implémente ce standard nativement. Dans notre architecture, un outil `McpTool` est considéré comme une `PhysicalFunction` dans l'ontologie, ancrant l'exécution technique dans le modèle d'ingénierie système.

---

## 🏗️ Architecture Technique

### Le Trait `AgentTool`

Cœur du module, ce contrat définit comment le moteur communique avec le Jumeau Numérique ou les APIs système. La nouvelle signature intègre nativement le contexte d'exécution :

```rust
#[async_interface]
pub trait AgentTool: Send + Sync + FmtDebug {
    // Identifiant unique (ex: "read_system_metrics")
    fn name(&self) -> &str;           
    
    // Manuel d'utilisation pour l'Orchestrateur IA
    fn description(&self) -> &str;    
    
    // Validation JSON Schema des entrées attendues
    fn parameters_schema(&self) -> JsonValue; 
    
    // Logique métier asynchrone exploitant le Jumeau Numérique via le contexte
    async fn execute(&self, args: &JsonValue, context: &HandlerContext<'_>) -> RaiseResult<JsonValue>;
}
```

### Cycle de vie d'une exécution

1. **Injection Dynamique** : Lors de la compilation du Mandat, le compilateur lit la base de données (`ref:configs:tool_dependencies`) pour injecter l'outil requis par une règle métier.
2. **Déclenchement** : Un nœud `CallMcp` est atteint dans le graphe d'exécution.
3. **Exécution Stateless** : L'implémentation Rust utilise `context.manager` pour lire ou écrire dans la base (ex: collection `digital_twin`) sans overhead d'initialisation.
4. **Persistance** : Le résultat est injecté dans le `context` du workflow sous la clé spécifiée (`output_key`), le rendant disponible pour les nœuds suivants (ex: `GatePolicy` ou `Task`).

---

## 🚀 Guide de Développement : Créer un Outil Data-Driven

La création d'un outil est désormais extrêmement simplifiée grâce au `CollectionsManager` injecté.

### 1. Définition de la logique (Exemple : `SystemMonitorTool`)

L'outil lit directement les capteurs depuis la collection `digital_twin` de la base de données :

```rust
#[async_interface]
impl AgentTool for SystemMonitorTool {
    fn name(&self) -> &str { "read_system_metrics" }
    
    fn description(&self) -> &str { "Lit les valeurs temps réel des capteurs du Jumeau Numérique." }
    
    fn parameters_schema(&self) -> JsonValue {
        json_value!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, _args: &JsonValue, context: &HandlerContext<'_>) -> RaiseResult<JsonValue> {
        // Lecture ultra-rapide via la JSON-DB mutualisée
        let vibration_z = match context.manager.get_document("digital_twin", "vibration_z").await {
            Ok(Some(doc)) => doc["value"].as_f64().unwrap_or(0.0),
            _ => 0.0
        };

        Ok(json_value!({
            "vibration_z": vibration_z,
            "status": "ONLINE"
        }))
    }
}
```

### 2. Enregistrement Système

L'outil doit être déclaré dans le `WorkflowExecutor` lors de l'initialisation du moteur :

```rust
// Dans l'initialisation de votre application
let mut executor = WorkflowExecutor::new(orchestrator, plugin_manager);
executor.register_tool(Box::new(SystemMonitorTool::new()));
```

---

## 🛡️ Sécurité et "Lignes Rouges" (Vetos)

L'intégration d'un outil MCP dans un workflow est souvent la première étape d'un **Veto automatique** (Ligne Rouge d'un Mandat) :

1. **Routage Data-Driven** : Le compilateur détecte une règle active (ex: `VIBRATION_MAX`) et injecte automatiquement le nœud `CallMcp` correspondant à `read_system_metrics`.
2. **Acquisition** : Le `McpHandler` s'exécute et stocke la valeur du capteur dans le contexte du workflow (ex: sous la variable `sensor_vibration`).
3. **Évaluation** : Le nœud suivant (`GatePolicy`) évalue un arbre syntaxique strict (AST) sur cette variable. Si le seuil de tolérance est dépassé, le moteur interrompt immédiatement l'exécution via un _Fail-Safe_.
```