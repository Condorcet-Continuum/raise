#!/usr/bin/env bash
# Génère les SBOMs (Rust + JS + Bundles) sans warnings à l'écran.
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT_DIR"

OUT_DIR="target/sbom"
mkdir -p "$OUT_DIR"

# ---- Tooling checks (silencieux) ----
command -v cargo >/dev/null || { echo "❌ cargo introuvable"; exit 1; }
command -v syft  >/dev/null || { echo "❌ syft introuvable (installe-le dans ~/.local/bin et ajoute-le au PATH)"; exit 1; }

# Version projet pour tagger les SBOMs proprement
PROJ_VER="$(git describe --tags --always 2>/dev/null || echo 0.1.0)"

# -------------------------------------
# 1) SBOM Rust (Tauri)
# -------------------------------------
if [ -f src-tauri/Cargo.toml ]; then
  # quiet + on masque les sorties non utiles
  cargo cyclonedx --format json --manifest-path src-tauri/Cargo.toml -q 1>/dev/null 2>>"$OUT_DIR/sbom_local.log" || true
  if [ -f src-tauri/bom.json ]; then
    mv -f src-tauri/bom.json "$OUT_DIR/sbom-rust-tauri.cdx.json"
  fi
fi

# -------------------------------------
# 2) SBOM Rust (WASM) si présent
# -------------------------------------
if [ -f src-wasm/Cargo.toml ]; then
  cargo cyclonedx --format json --manifest-path src-wasm/Cargo.toml -q 1>/dev/null 2>>"$OUT_DIR/sbom_local.log" || true
  if [ -f src-wasm/bom.json ]; then
    mv -f src-wasm/bom.json "$OUT_DIR/sbom-rust-wasm.cdx.json"
  fi
fi

# -------------------------------------
# 3) SBOM JS (repo)
#    -o FILE + source-name/version pour éviter le WARN "no explicit name"
#    SYFT_LOG=error + pas de check update pour éviter les WARN
# -------------------------------------
SYFT_ENV_COMMON=(SYFT_LOG=error SYFT_CHECK_FOR_APP_UPDATE=false)
"${SYFT_ENV_COMMON[@]}" syft dir:. \
  -o "cyclonedx-json=$OUT_DIR/sbom-js.cdx.json" \
  --source-name "genaptitude" \
  --source-version "$PROJ_VER" \
  1>/dev/null 2>>"$OUT_DIR/sbom_local.log"

# -------------------------------------
# 4) SBOM des bundles Tauri (si présents)
# -------------------------------------
if [ -d target/release/bundle ]; then
  "${SYFT_ENV_COMMON[@]}" syft dir:target/release/bundle \
    -o "cyclonedx-json=$OUT_DIR/sbom-bundles.cdx.json" \
    --source-name "genaptitude-bundles" \
    --source-version "$PROJ_VER" \
    1>/dev/null 2>>"$OUT_DIR/sbom_local.log" || true
fi

# -------------------------------------
# 5) Récap propre
# -------------------------------------
echo "✅ SBOMs générés dans $OUT_DIR :"
ls -lh "$OUT_DIR"/*.json 2>/dev/null || true
echo "ℹ️  Logs détaillés (warnings masqués à l'écran) : $OUT_DIR/sbom_local.log"
