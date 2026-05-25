# 🧠 RAISE Foundation - Module `context` (État Global & Observabilité)

Bienvenue dans le cerveau de l'application. Le sous-domaine `src-tauri/src/utils/context` gère l'**État Global d'Exécution**. Il répond à trois questions fondamentales à tout instant "t" :
1. **Qui utilise l'application ?** (Identité, Permissions, Contexte Base de Données) -> `session.rs`
2. **Dans quelle langue ?** (Internationalisation, Traduction dynamique) -> `i18n.rs`
3. **Que se passe-t-il ?** (Télémétrie, Tracing, Audit) -> `logger.rs`

---

## 🛑 DIRECTIVES SYSTÈMES STRICTES (POUR LES AGENTS IA)

1. **Observabilité RAISE (Zéro Tracing Brut)** : Dans le code métier, vous ne devez **JAMAIS** importer ou utiliser les macros brutes de la crate `tracing` (`info!`, `error!`, `warn!`, `debug!`). 
   * Vous **devez** utiliser les macros encapsulées de la fondation : `user_info!`, `user_error!`, `user_warn!`, `user_debug!` et `raise_error!`.
   * Ces macros garantissent l'injection du contexte JSON, de l'ID de corrélation et des traductions i18n.
2. **Verrous Explicites** : L'état global est partagé entre plusieurs threads (Tauri, requêtes réseau, tâches de fond). Les accès concurrents sont protégés par `SharedRef`, `AsyncRwLock` ou `SyncRwLock`. N'essayez jamais de contourner ces verrous ou de dupliquer l'état en mémoire.
3. **Aucun `unwrap()` sur les Verrous** : Les verrous peuvent être empoisonnés (poisoned) en cas de panic d'un thread. Gérez toujours l'erreur proprement via `RaiseResult`.

---

## 1. 🛡️ Gestion des Sessions (`session.rs`)

Le gestionnaire de session (`SessionManager`) est la source de vérité pour l'authentification et le contexte de sécurité de l'utilisateur actif.

* **Persistance** : Toute session démarrée (`start_session`) ou mise à jour (`touch`) est automatiquement synchronisée avec la base de données interne `json_db` dans la collection `sessions`.
* **Cycle de vie** : Les sessions ont une durée de vie (par défaut 8 heures) et un état (`Active`, `Idle`, `Expired`, `Revoked`).
* **Contexte de Routage** : Le `SessionContext` contient `current_domain`, `current_db` et `active_dapp`. C'est grâce à lui que le système sait dans quelle base de données écrire les opérations de l'utilisateur.

### 💡 Règle d'implémentation métier :
Ne réinventez pas la vérification de session. Pour protéger une commande Tauri ou une fonction métier critique, utilisez la macro dédiée (si elle est définie dans la fondation) ou récupérez la session active via `SessionManager::get_current_session().await`.

---

## 2. 🌍 Internationalisation (`i18n.rs`)

Le système de traduction (`Translator`) est un singleton global maintenu en mémoire vive via un `StaticCell` pour un accès instantané (zéro I/O disque lors de la lecture).

* **Chargement** : Les traductions sont stockées dynamiquement dans la base de données système (`_system/locales`). La fonction `init_i18n("fr")` va chercher le document JSON correspondant et le charger en RAM.
* **Fonction `t(key)`** : C'est le point d'entrée universel. Si une clé n'est pas trouvée, la fonction retourne la clé elle-même (ex: "MSG_UNKNOWN") plutôt que de faire crasher l'application, assurant une résilience totale de l'UI.
* **Couplage Macros** : Toutes les macros d'observabilité (`user_info!`, `raise_error!`, etc.) appellent automatiquement `i18n::t()` en interne. Ne traduisez jamais manuellement un message avant de le passer à une macro.

---

## 3. 📡 Observabilité et Logging (`logger.rs`)

Le `logger.rs` est la salle des machines qui configure les flux de télémétrie.

* **Rotation des Logs** : Les logs sont écrits de manière asynchrone dans le dossier `_system/logs/` avec une rotation quotidienne (`raise.log.2026-03-08`).
* **Format AI-Ready (JSON)** : Le fichier de log n'est pas du texte brut, c'est du **JSONL** (JSON Lines). Chaque ligne est un objet parsable nativement par une IA pour du débogage automatisé, incluant les champs `service`, `component`, `event_id`, et le `context` structuré.
* **Idempotence** : L'initialisation du logger utilise `InitGuard` (Once). Appeler `init_logging()` plusieurs fois est sans danger (no-op).