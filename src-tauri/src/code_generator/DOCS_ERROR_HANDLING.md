# 📜 Condorcet-Continuum : Error Design Protocol (V1.3)

## 🎯 Philosophie

L'objectif n'est pas seulement d'empêcher les crashs, mais de fournir une **télémétrie granulaire** exploitable par des Agents AI et des humains. Chaque erreur doit être un levier de diagnostic.

## 🛠 Les Deux Outils de Pouvoir

### 1. `raise_error!` (Le Scalpel)

**Quand :** Dans les blocs `match` pour les opérations critiques (IA, Blockchain, I/O).
**Effet :** Construit l'erreur et effectue un `return Err(...)` immédiat.
**Pourquoi :** Offre un espace visuel pour un contexte JSON riche.

```rust
match operation_critique() {
    Ok(val) => val,
    Err(e) => raise_error!(
        "ERR_CODE_UNIQUE",
        error = e,
        context = json!({ "detail": "clé", "action": "explication" })
    )
}

```

### 2. `build_error!` (La Jointure)

**Quand :** Dans les clôtures `.map_err()`.
**Effet :** Retourne uniquement l'objet `AppError`.
**Pourquoi :** Permet de maintenir le chaînage (`?`) sur des opérations de "plomberie" simples.

```rust
let path = str_path.parse().map_err(|e| build_error!("ERR_PATH_INVALID", error = e))?;

```

---

## 🏗 Règles d'Or pour l'IA

### A. La Règle du Contexte JSON

Ne jamais envoyer une erreur "nue". Le champ `context` doit au moins contenir :

* `action`: Ce que le système essayait de faire.
* `hint`: Une piste de résolution (ex: "Vérifiez Docker").
* `state`: (Si possible) Les dimensions des tenseurs ou les IDs concernés.

### B. Anti-Pattern : L'Oignon de Result

Ne jamais utiliser `.map_err(|e| raise_error!(...))?`.

* **Pourquoi :** Cela crée un `Result<T, Result<E, AppError>>` qui brise l'inférence de type de l'Agent AI.
* **Correction :** Passer au format `match` + `raise_error!`.

### C. Le Code d'Erreur est une Clé

Utiliser des préfixes constants pour faciliter le filtrage :

* `ERR_AI_...` : Moteurs Candle, Tenseurs, Optimiseurs.
* `ERR_BLOCKCHAIN_...` : gRPC, Chaincode, Fabric.
* `ERR_SYS_...` : Mutex, I/O, Système de fichiers.

---

## 🤖 Note pour les Agents AI

Lors de la génération de code pour Condorcet, privilégiez la **clarté structurelle** (`match`) sur la **concision fonctionnelle**. Un bloc d'erreur explicite est préférable à un chaînage obscur. Si une opération implique plus de deux transformations, décomposez-la.

---

 