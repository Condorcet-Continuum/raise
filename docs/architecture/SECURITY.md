# SECURITY — GenAptitude
**Version :** 1.0 · **Date :** 2025-11-08 · **Périmètre :** Repo GenAptitude (Tauri v2 + Rust + WASM + Vite/React)  
**Contact vulnérabilités :** security@genaptitude.example (ou *GitLab → Security → New advisory*)

---

## 1) Modèle de menace & périmètre
- **Workstation-first** (poste local) avec packaging desktop. Risques : fuite de secrets, supply-chain, élévation locale, XSS/CSP, exécutions non fiables.
- Hypothèses : poste à jour, utilisateur non malveillant, CI GitLab de confiance.
- Données sensibles : documents d’entreprise, prompts, journaux, éventuels secrets d’accès locaux.

---

## 2) Signalement de vulnérabilités
- **Privé d’abord** : envoyez un mail à *security@genaptitude.example* avec reproduction minimale, POC, impact.
- Délai d’accusé de réception : **72h** ; correctif : **≤30 jours** si critique.
- Merci d’éviter les tickets publics avant correctif et fenêtre de patch.

---

## 3) Gestion des secrets
- **Ne jamais** commiter de secrets (`.env`, tokens, clés) ; ajouter aux `.gitignore` si besoin.
- Utiliser le **keyring OS** côté desktop. Côté CI : **CI/CD → Variables** (masquées, protégées).
- Logs et traces : **pas** de secrets/PII. Redaction côté app/OTel si doute.
- Rotation trimestrielle recommandée (SSH, tokens CI, clés de signature).

---

## 4) Durcissement de l’app (Tauri/Frontend)
- **CSP** stricte, pas d’évaluations dynamiques ; désactiver ce qui n’est pas nécessaire.
- **Allowlist Tauri** : autoriser uniquement les commandes/invokes nécessaires.
- **Pas de code distant** non signé ; **aucun `eval`** ; limiter `open`/`opener`.
- **Pages statiques** : servies localement, liens absolus `/pages/...` ; pas de scripts non maîtrisés.
- **WASM** : exécutions non fiables en **WASI** sans accès FS/réseau par défaut.

---

## 5) Supply-chain & dépendances
- **Rust** : toolchain fixé ; `cargo update -p <pkg>` ciblé ; `cargo audit` (RUSTSEC) à chaque release.
- **TS/Node** : lockfile ; `npm ci` (ou pnpm/yarn locked) ; `npm audit` (ou `pnpm audit`).
- Politique de licences : `cargo deny` (denylist/allowlist), revue des transitive deps.
- **Gitleaks/TruffleHog** pré-push (secret scanning).

```bash
# Rust
cargo install cargo-audit cargo-deny cargo-outdated
cargo audit && cargo deny check && cargo outdated || true

# JS
npm audit --production || true

# Secret scanning
gitleaks detect --no-git -v || true
```

---

## 6) SBOM & attestation
- Générer un **SBOM CycloneDX** (Rust + JS) et l’attacher aux artefacts de release.
```bash
# Rust
cargo install cargo-cyclonedx
cargo cyclonedx --format json --output target/sbom-rust.cdx.json

# JS (si syft dispo)
syft packages dir:. -o cyclonedx-json > target/sbom-js.cdx.json
```
- Signer/attester les SBOM avec **cosign** (optionnel) : `cosign sign-blob` / `cosign attest`.

---

## 7) Build & artefacts sécurisés
- Builds **reproductibles** en CI (toolchain fixée, images épinglées).
- Signer **AppImage/.deb/.rpm** et publier les **SHA256**.
```bash
# Hash
sha256sum target/release/bundle/**/* 2>/dev/null | tee SHA256SUMS.txt

# Signature (ex. cosign key-pair pré-installé)
cosign sign-blob --key cosign.key target/release/bundle/appimage/GenAptitude_*.AppImage > appimage.sig
```
- (Deb/RPM) Signatures natives possibles : `dpkg-sig`, `rpm --addsign`.

---

## 8) Journalisation, PII & Observabilité
- **OTel** activable par env var ; logs **JSON** sans PII/secrets ; niveau par défaut = INFO.
- Rétention locale courte (ex. 7 jours). Export vers Prometheus/Loki **opt-in** seulement.
- Droit à l’oubli : effacer `{{app_data_dir}}/evidence/*` et caches locaux à la demande.

---

## 9) Politique de correctifs
- Critique (RCE/fuite) : hotfix + release ≤ **72h** ; communication aux utilisateurs.
- Haute : correctif ≤ **7 jours** ; Moyenne/Basse : embarquées dans prochaine mineure.
- Documenter dans **CHANGELOG.md** (Security).

---

## 10) Réponse à incident (extrait)
1. **Isoler** la machine/runner affecté.  
2. **Révoquer** tokens/clé compromettus (CI, SSH).  
3. **Auditer** artefacts & logs (recherche IoC).  
4. **Corriger** et **publier** version patchée + avis sécurité.  
5. **Post-mortem** (causes, actions, prévention).

---

## 11) Checklist sécurité (extrait)
- [ ] Secrets hors repo, variables CI masquées.  
- [ ] CSP/allowlist Tauri configurées.  
- [ ] `cargo audit`, `cargo deny`, `npm audit` verts.  
- [ ] SBOM générés et signés.  
- [ ] SHA256 + signatures publiées.  
- [ ] Pre-push **gitleaks** OK.  
- [ ] OTel sans PII.
