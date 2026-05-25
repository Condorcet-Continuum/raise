# 📂 RAISE Foundation - Module `io` (Fichiers, OS & Sandboxing)

Bienvenue dans le module des "Effets de Bord" (Side-Effects) du framework RAISE. Ce sous-domaine (`src-tauri/src/utils/io`) est la **seule et unique façade autorisée** pour interagir avec le système d'exploitation hôte (système de fichiers, exécution de processus, flux standards, compression).

Il encapsule la complexité asynchrone (`tokio`), sécurise les accès via Sandboxing, garantit l'intégrité des données via des écritures atomiques, et assure une observabilité structurée "AI-Ready" des erreurs.

---

## 🛑 DIRECTIVES SYSTÈMES STRICTES (POUR LES AGENTS IA)

1. **Le Prélude est Roi** : Dans les services métiers (`src/ai`, `src/blockchain`, etc.), n'importez **JAMAIS** `std::fs`, `tokio::fs`, ou `std::process::Command` directement. Utilisez exclusivement `use crate::utils::prelude::*;`.
2. **Zéro Runtime Brut** : N'utilisez **JAMAIS** `tokio::task::spawn_blocking`. Passez par l'alias sémantique `spawn_cpu_task` fourni par le framework.
3. **Vocabulaire RAISE** : Oubliez les types standards. Utilisez `ProcessCommand` (pas `Command`), `CompressionEncoder` (pas `zstd::Encoder`), `stdout_raw` (pas `std::io::stdout`).

---

## 1. L'Architecture Interne (La Forteresse)

Le module est divisé en composants physiques stricts, tous ré-exportés par la façade `io/mod.rs` :
* `fs.rs` : Système de fichiers (Async/Sync, Sandboxing, JSON, Bincode).
* `os.rs` : Exécution de processus système et flux I/O terminaux.
* `compression.rs` : Algorithmes de compression (Zstd).
* `io_traits.rs` : Alias sémantiques des traits standards (`SyncRead`, `SyncWrite`).
* `os_types.rs` : Alias sémantiques des types systèmes (`ProcessCommand`, `ProcessOutput`).

---

## 2. La Loi de l'Explicite : `_async` vs `_sync`

Pour éliminer 100% des ambiguïtés liées au threading, l'API impose un suffixe strict sur **toutes** ses fonctions.

* **Les fonctions `_async`** : Elles délèguent le travail au runtime (ex: `tokio`). Elles **doivent** être suivies d'un `.await`. À utiliser **partout** dans la logique métier, les commandes Tauri et le réseau pour ne pas geler l'application.
  * *Exemple* : `read_json_async`, `write_atomic_async`.
* **Les fonctions `_sync`** : Elles bloquent le thread courant de l'OS. Elles sont **strictement réservées** aux phases d'initialisation (démarrage de l'app, `OnceLock`, `StaticCell`) ou aux scripts utilitaires CLI.
  * *Exemple* : `read_json_sync`, `write_atomic_sync`.

---

## 3. Sécurité et Atomicité (Le standard `fs.rs`)

L'écriture de données critiques (JSON, bases locales) est sujette à des corruptions (coupure de courant, espace disque plein). 

### ✅ La Règle d'Écriture Atomique
Pour sauvegarder un état persistant, utilisez exclusivement les fonctions `_atomic`. Elles écrivent dans un fichier `.tmp`, forcent la synchronisation matérielle (`sync_all`), puis renomment le fichier final de manière instantanée.
* `write_atomic_async(path, data)`
* `write_json_atomic_async(path, data)`
* `write_compressed_atomic_async(path, data)`

### 🛡️ Le Sandboxing : `ProjectScope`
Si vous manipulez des chemins provenant d'inputs utilisateurs ou du réseau, utilisez **impérativement** `ProjectScope`. Il empêche les évasions mortelles de type "Path Traversal" (ex: `../../etc/passwd`).
```rust
let scope = ProjectScope::new_sync("/data/safe_zone")?;

// ✅ Autorisé
scope.write_async("user_data.json", b"{}").await?;

// ❌ Rejeté (ERR_FS_SECURITY_VIOLATION) avant même de toucher le disque
scope.write_async("../secret.txt", b"hack").await?;

```

---

## 4. Couche Système (`os.rs`)

Encapsule le lancement de processus enfants via `ProcessCommand`.

* **`exec_command(cmd, args, cwd)`** : Lance un processus OS. L'observabilité est totale : elle émet un `user_debug!` au lancement, et en cas d'échec, elle lève une erreur `Structured` contenant le flux `stderr` complet et le code de sortie (`exit_code`) pour faciliter l'auto-réparation par l'IA.
* **`pipe_through(cmd, input)`** : Envoie des flux texte via `stdin` vers des outils externes et récupère `stdout` (idéal pour formater du code via CLI).
* **Flux Standards** : Utilisez `flush_stdout()`, `read_stdin_line()` ou `prompt(msg)` pour interagir avec le terminal hôte sans importer `std::io`.

---

## 5. Compression et Délégation CPU (`compression.rs`)

Le framework utilise **Zstd (Niveau 3)** pour un équilibre parfait entre vitesse et taille.

* **Délégation CPU** : La compression étant "CPU-Bound", les fonctions comme `write_compressed_atomic_async` délèguent automatiquement la charge de travail matérielle via `spawn_cpu_task`. Cela garantit que la boucle d'événements principale (`tokio`) n'est jamais bloquée par une grosse compression JSON.
* **Transparence** : L'utilisation de `read_json_compressed_async<T>(path)` lit, décompresse, vérifie l'UTF-8 et désérialise le JSON en une seule ligne métier.

```
