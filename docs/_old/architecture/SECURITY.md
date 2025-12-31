# SECURITY — GenAptitude

**Version :** 1.1 · **Date :** 2025-11-29 · **Périmètre :** Repo GenAptitude (Tauri v2 + Rust + WASM + Vite/React)  
**Contact vulnérabilités :** security@genaptitude.example (ou _GitLab → Security → New advisory_)

---

## 1) Modèle de menace & périmètre

- **Workstation-first** (poste local) avec packaging desktop.
- **Risques identifiés** :
  - Fuite de secrets (tokens API, clés privées).
  - Supply-chain (dépendances compromises).
  - Élévation locale (permissions fichiers).
  - **Injection de données** : Corruption via fichiers JSON-DB malformés ou schémas malveillants.
  - **Déni de service (DoS)** : Boucles infinies dans l'expansion JSON-LD ou surcharge mémoire du `StorageEngine`.
- **Données sensibles** : Documents d’ingénierie (IP), historique de chat, journaux d'audit (`_wal.jsonl`).

---

## 2) Signalement de vulnérabilités

- **Privé d’abord** : envoyez un mail à *security@genaptitude.example* avec reproduction minimale, POC, impact.
- Délai d’accusé de réception : **72h** ; correctif : **≤30 jours** si critique.
- Merci d’éviter les tickets publics avant correctif et fenêtre de patch.

---

## 3) Gestion des secrets

- **Ne jamais** commiter de secrets (`.env`, tokens, clés) ; ajouter aux `.gitignore`.
- Utiliser le **keyring OS** côté desktop pour les clés sensibles (ex: identité Fabric).
- **Configuration** : Les chemins critiques (`PATH_RAISE_DOMAIN`) sont configurés par variables d'environnement ou defaults sécurisés.
- Rotation trimestrielle recommandée (SSH, tokens CI, clés de signature).

---

## 4) Durcissement de l’app (Tauri/Frontend)

- **CSP** stricte, pas d’évaluations dynamiques ; désactiver ce qui n’est pas nécessaire.
- **Allowlist Tauri** : autoriser uniquement les commandes IPC nécessaires (`jsondb_*`, `load_project_model`, etc.).
- **Pas de code distant** non signé ; **aucun `eval`** dans le frontend.
- **WASM** : exécutions non fiables en **WASI** sans accès FS/réseau par défaut.
- **Model Engine** : Le chargement de modèles lourds est isolé dans des threads dédiés (`spawn_blocking`) pour éviter le gel de l'interface (DoS local).

---

## 5) Sécurité des Données (JSON-DB & Model Engine)

- **Validation Stricte** : Aucune donnée n'est persistée sans validation conforme au **JSON Schema** associé (Draft 2020-12).
- **Moteur `x_compute`** : L'exécution des règles de calcul (ID, timestamps) utilise un interpréteur restreint (pas de `eval` JS/Rust), limitant les risques d'exécution de code arbitraire (RCE).
- **Intégrité (ACID)** : Utilisation d'un **Write-Ahead Log (WAL)** pour garantir l'intégrité des données en cas de crash.
- **Sémantique** : Le `ModelLoader` rejette les types JSON-LD inconnus ou malformés, empêchant l'injection d'objets non-métier dans la mémoire.

---

## 6) Supply-chain & dépendances

- **Rust** : toolchain fixé ; `cargo audit` (RUSTSEC) à chaque release.
- **TS/Node** : lockfile ; `npm audit` (ou `pnpm audit`).
- Politique de licences : `cargo deny` (denylist/allowlist).
- **Gitleaks/TruffleHog** pré-push (secret scanning).

```bash
# Rust
cargo install cargo-audit cargo-deny cargo-outdated
cargo audit && cargo deny check && cargo outdated || true

# JS
npm audit --production || true
```

---

## 7\) SBOM & attestation

- Générer un **SBOM CycloneDX** (Rust + JS) et l’attacher aux artefacts de release.

<!-- end list -->

```bash
# Rust
cargo cyclonedx --format json --output target/sbom-rust.cdx.json
# JS
syft packages dir:. -o cyclonedx-json > target/sbom-js.cdx.json
```

- Signer les artefacts avec **cosign**.

---

## 8\) Build & artefacts sécurisés

- Builds **reproductibles** en CI (toolchain fixée, images épinglées).
- Signer **AppImage/.deb/.rpm** et publier les **SHA256**.

<!-- end list -->

```bash
# Hash
sha256sum target/release/bundle/**/* 2>/dev/null | tee SHA256SUMS.txt
```

---

## 9\) Journalisation, PII & Observabilité

- **OTel** activable par env var ; logs **JSON** sans PII/secrets.
- **DB Logs** : Le fichier `_wal.jsonl` contient l'historique des transactions. Il doit être protégé par les permissions OS (lecture seule pour l'utilisateur).
- **Droit à l’oubli** : La suppression d'un document via `jsondb_delete` entraîne sa suppression physique et son invalidation dans le cache.

---

## 10\) Réponse à incident (extrait)

1.  **Isoler** la machine/runner affecté.
2.  **Révoquer** tokens/clés compromis.
3.  **Auditer** les journaux JSON-DB (`_wal.jsonl`) pour détecter des injections.
4.  **Corriger** et publier version patchée.

---

## 11\) Checklist sécurité (extrait)

- [ ] Secrets hors repo, variables CI masquées.
- [ ] CSP/allowlist Tauri configurées.
- [ ] Validation des schémas JSON stricte activée.
- [ ] `cargo audit` / `cargo deny` verts.
- [ ] SBOM générés et signés.
- [ ] SHA256 + signatures publiées.
