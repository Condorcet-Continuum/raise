Voici le fichier `RELEASING.md` remis au format Markdown complet, propre et prêt à être utilisé.

---

# RELEASING — GenAptitude

**Version :** 1.1 · **Date :** 2025-11-29 · **Audience :** Maintainers

---

## 1\) Versioning & branches

- **SemVer** : `MAJOR.MINOR.PATCH` (phase 0.x : API instable, _minor_ = breaking possible).
- Branche **main** protégée ; releases par **tags** `vX.Y.Z`.
- `tauri.conf.json` **version** = version de référence des artefacts.

---

## 2\) Pré-release checklist

- [ ] Pipeline **vert** sur `main` (lint/build/test/bundle).
- [ ] **Tests d'intégration** : `cargo test --test json_db_suite` passe à 100%.
- [ ] **Model Engine** : Validation sémantique (JSON-LD) vérifiée via `cargo test --lib model_engine`.
- [ ] `cargo audit` / `cargo deny` / `npm audit` OK (ou risques documentés).
- [ ] **CHANGELOG.md** mis à jour (Added/Changed/Fixed/Security).
- [ ] **SBOM** générés (`target/sbom-*.cdx.json`).
- [ ] **Identifier Tauri** vérifié (éviter suffixe `.app` sur macOS).

---

## 3\) Bump de version

- Ajuster `src-tauri/tauri.conf.json` → `version` = `X.Y.Z`.
- Ajuster `src-tauri/Cargo.toml` → `version` = `X.Y.Z`.
- Commit :

<!-- end list -->

```bash
git add src-tauri/tauri.conf.json src-tauri/Cargo.toml
git commit -m "chore(release): vX.Y.Z"
```

---

## 4\) Tag & publication CI

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

- Le pipeline sur le **tag** construit :
  - **Web** (`dist/`)
  - **WASM** (`target/wasm32-*/release/*.wasm`)
  - **Desktop** (**AppImage/.deb/.rpm** sous `target/release/bundle/**`)

---

## 5\) Hash, signatures & SBOM

Après pipeline (local ou job dédié) :

```bash
# Hash
find target/release/bundle -type f -maxdepth 3 -print0 | xargs -0 sha256sum | tee SHA256SUMS.txt

# SBOM (si pas déjà faits en CI)
cargo cyclonedx --format json --output target/sbom-rust.cdx.json
syft packages dir:. -o cyclonedx-json > target/sbom-js.cdx.json  # option

# Signatures (ex. cosign)
cosign sign-blob --key cosign.key target/release/bundle/appimage/GenAptitude_*.AppImage > appimage.sig
cosign attest --predicate target/sbom-rust.cdx.json --type cyclonedx target/release/bundle/appimage/GenAptitude_*.AppImage
```

- Uploader **SHA256SUMS.txt**, signatures et SBOM avec les artefacts.

---

## 6\) Notes de release (modèle)

```
## GenAptitude vX.Y.Z — YYYY-MM-DD (Europe/Paris)

### Added
- Model Engine : Support sémantique JSON-LD (OA, SA, LA, PA).
- JSON-DB : Transactions ACID et Indexation.

### Changed
- …

### Fixed
- …

### Security
- Dependency {pkg} → {version} (CVE-…)
- CSP hardened; allowlist Tauri tightened.
```

---

## 7\) Smoke tests post-release

- **Installation** : `dpkg -i` (Linux) ou exécution AppImage.
- **Persistance** : Vérifier que le dossier `$PATH_RAISE_DOMAIN` (ou défaut) est créé et accessible en écriture.
- **Model Loading** : Lancer l'app et vérifier que le modèle par défaut se charge sans erreur (voir logs console).
- **WASM** : Vérifier que le module `ga_wasm` est bien chargé par le frontend.

---

## 8\) Rollback

- Re-créer un tag **vX.Y.(Z-1)** en “latest” si régression majeure.
- Communiquer rapidement (release notes _Rollback_), lister impacts & mitigations.
- Si corruption de données (DB), fournir la procédure de restauration (`docs/architecture/operational-runbook.md`).

---

## 9\) Automatisation (idées)

- Job **release** dédié sur tag : hash + cosign + upload notes.
- Génération automatique du **CHANGELOG** (conventional commits).
- **SLSA provenance** via cosign attestations.
