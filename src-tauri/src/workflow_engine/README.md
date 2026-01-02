# 🔀 Module Workflow Engine

Ce module implémente le moteur d'orchestration **symbolique** de RAISE.
Il est responsable de l'exécution déterministe des plans d'actions, qu'ils soient définis manuellement par un ingénieur ou générés dynamiquement par un Agent IA.

Contrairement aux agents (qui sont "créatifs" et probabilistes), le Workflow Engine est **rigide et auditable**.

---

## 🏗️ Architecture

Le moteur repose sur une séparation stricte des responsabilités en trois composants :

| Composant         | Fichier            | Rôle                                                                                                                                             |
| :---------------- | :----------------- | :----------------------------------------------------------------------------------------------------------------------------------------------- |
| **Scheduler**     | `scheduler.rs`     | **Chef d'orchestre**. Il maintient le registre des définitions de workflow et pilote le cycle de vie des instances (Start, Step, Pause, Resume). |
| **State Machine** | `state_machine.rs` | **Navigateur**. Il analyse le graphe (Nœuds + Liens) pour déterminer quels sont les prochains nœuds éligibles en fonction de l'état actuel.      |
| **Executor**      | `executor.rs`      | **Ouvrier**. Il exécute une tâche unitaire (Appel API, Agent IA, Calcul) sans se soucier du reste du graphe.                                     |

---

## 🧩 Modèle de Données

Le moteur manipule deux concepts distincts :

1.  **Définition (`WorkflowDefinition`)** : Le "Moule" statique (JSON). Il contient la liste des nœuds et des arêtes (edges). Il est immuable.
2.  **Instance (`WorkflowInstance`)** : L'exécution dynamique. Elle contient l'état de chaque nœud (`Pending`, `Running`, `Completed`), les logs et le contexte de données (variables).

### Types de Nœuds Supportés

| Type           | Description       | Comportement                                                                      |
| :------------- | :---------------- | :-------------------------------------------------------------------------------- |
| **`Task`**     | Tâche standard    | Exécute une action (ex: Appel IA) puis passe à `Completed`.                       |
| **`Decision`** | Branchement       | Évalue une condition pour choisir la branche de sortie.                           |
| **`Parallel`** | Fork              | Lance plusieurs branches simultanément.                                           |
| **`GateHitl`** | Human-In-The-Loop | **Met le workflow en PAUSE**. Attend une intervention humaine via l'API `resume`. |
| **`CallMcp`**  | Tool Call         | Appelle un outil externe via le protocole MCP (Model Context Protocol).           |

---

## 🔄 Cycle de Vie d'une Exécution

### 1. Démarrage (`start_workflow`)

Une nouvelle `WorkflowInstance` est créée à partir d'une définition. Son statut est `Pending`.

### 2. Boucle d'Exécution (`run_step`)

Le Scheduler entre dans une boucle :

1.  Il demande à la **State Machine** : _"Quels sont les prochains nœuds ?"_
2.  Si la liste est vide : Le workflow est terminé (`Completed`).
3.  Sinon, pour chaque nœud :
    - Il délègue l'exécution à l'**Executor**.
    - Il met à jour le statut du nœud dans l'instance.

### 3. Gestion de la Pause (`GateHitl`)

Si l'Executor rencontre un nœud de type `GateHitl` (Validation Humaine) :

1.  Il retourne un statut `Paused`.
2.  Le Scheduler arrête immédiatement la boucle d'exécution.
3.  L'instance reste figée dans l'état `Paused`.

### 4. Reprise (`resume_node`)

Lorsque l'utilisateur (via le Frontend) valide l'étape :

1.  La commande `resume_workflow` est appelée avec `approved: true/false`.
2.  Le Scheduler force le statut du nœud à `Completed` (ou `Failed`).
3.  La boucle d'exécution reprend son cours normal.

---

## 💻 Exemple d'Utilisation (Rust)

```rust
use crate::workflow_engine::{WorkflowScheduler, WorkflowInstance};

// 1. Initialisation
let mut scheduler = WorkflowScheduler::new();
scheduler.register_workflow(my_definition);

// 2. Démarrage
let mut instance = WorkflowInstance::new("mon-workflow-id", context);

// 3. Exécution (Async)
// Avance tant que possible, s'arrête si Pause ou Fin
scheduler.run_step(&mut instance).await?;

// 4. Reprise (si pause)
if instance.status == ExecutionStatus::Paused {
    scheduler.resume_node(&mut instance, "node-validation", true)?;
    // On relance la boucle après reprise
    scheduler.run_step(&mut instance).await?;
}
```

---

## 🔗 Intégration Tauri

Le moteur est exposé au Frontend via le module `commands::workflow_commands`.
L'état global est stocké dans un `Mutex<WorkflowStore>` géré par Tauri.

- **`register_workflow`** : Sauvegarde un graphe dessiné dans l'éditeur.
- **`start_workflow`** : Lance une instance.
- **`resume_workflow`** : Débloque une porte HITL.
- **`get_workflow_state`** : Permet au frontend de poller l'avancement.

<!-- end list -->

```

```
