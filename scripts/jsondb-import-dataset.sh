#!/usr/bin/env bash
# scripts/jsondb-import-dataset.sh
# Importe un dataset thématique/versionné en lisant son manifest.json
#
# Usage:
#   scripts/jsondb-import-dataset.sh --theme arcadia --version v1 <space> <db> [--repo-root=PATH] [--schemas=REL]
#
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/jsondb-import-dataset.sh --theme THEME --version VER <space> <db> [--repo-root=PATH] [--schemas=REL]

Args:
  <space>   Espace logique (ex: un2)
  <db>      Nom de la DB (ex: _system)

Options:
  --theme=THEME       Thème (ex: arcadia, agentique, uo, xai)
  --version=VER       Version (ex: v1, v2025-11-13)
  --repo-root=PATH    Racine du repo (détection automatique par git sinon)
  --schemas=REL       Chemin relatif des schémas (défaut: schemas/v1)

Dépendances: jq
USAGE
}

THEME=""
VER=""
REPO_ROOT=""
SCHEMAS_REL="schemas/v1"

if [[ $# -lt 2 ]]; then usage; exit 1; fi

# parse flags first
while [[ $# -gt 0 ]]; do
  case "$1" in
    --theme=*)
      THEME="${1#*=}"; shift ;;
    --version=*)
      VER="${1#*=}"; shift ;;
    --repo-root=*)
      REPO_ROOT="${1#*=}"; shift ;;
    --schemas=*)
      SCHEMAS_REL="${1#*=}"; shift ;;
    -*)
      usage; exit 1 ;;
    *)
      break ;;
  esac
done

if [[ -z "$THEME" || -z "$VER" ]]; then usage; exit 1; fi
if [[ $# -lt 2 ]]; then usage; exit 1; fi

SPACE="$1"; shift
DB="$1"; shift || true

# repo root
if [[ -z "$REPO_ROOT" ]]; then
  if git_root="$(git rev-parse --show-toplevel 2>/dev/null)"; then
    REPO_ROOT="$git_root"
  else
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
  fi
fi

# env .env
if [[ -f "$REPO_ROOT/.env" ]]; then
  echo "🔎 Loading env from: $REPO_ROOT/.env"
  set -a; source "$REPO_ROOT/.env"; set +a
fi
if [[ -z "${PATH_RAISE_DATASET:-}" ]]; then
  echo "❌ PATH_RAISE_DATASET non défini (.env)."; exit 1
fi

DATASET_DIR="$PATH_RAISE_DATASET/$THEME/$VER"
MANIFEST="$DATASET_DIR/manifest.json"
if [[ ! -f "$MANIFEST" ]]; then
  echo "❌ manifest.json introuvable: $MANIFEST"; exit 1
fi

SCHEMAS_ROOT="$REPO_ROOT/$SCHEMAS_REL"
echo "➡  Repo root     : $REPO_ROOT"
echo "➡  Schemas root  : $SCHEMAS_ROOT"
echo "➡  Dataset dir   : $DATASET_DIR"
echo "➡  Manifest      : $MANIFEST"

if ! command -v jq >/dev/null 2>&1; then
  echo "❌ jq requis (apt install jq ou pacman -S jq)"; exit 1
fi

run_cli() {
  if command -v jsondb >/dev/null 2>&1; then
    jsondb --repo-root "$REPO_ROOT" "$@"
  else
    ( set -x; cargo run -p jsondb_cli -- --repo-root "$REPO_ROOT" "$@" )
  fi
}

# open DB to ensure it's there
run_cli db open "$SPACE" "$DB" >/dev/null

# iterate collections from manifest
echo "==> Import via manifest…"
keys=$(jq -r '.collections | keys[]' "$MANIFEST")
for coll in $keys; do
  schema_rel=$(jq -r ".collections[\"$coll\"].schema" "$MANIFEST")
  dir_rel=$(jq -r ".collections[\"$coll\"].dir" "$MANIFEST")

  schema_path="$SCHEMAS_ROOT/$schema_rel"
  data_dir="$DATASET_DIR/$dir_rel"

  if [[ ! -f "$schema_path" ]]; then
    echo "⚠️  Schéma absent → skip: $schema_rel"; continue
  fi
  if [[ ! -d "$data_dir" ]]; then
    echo "⚠️  Dossier données absent → skip: $data_dir"; continue
  fi

  shopt -s nullglob
  files=( "$data_dir"/*.json )
  shopt -u nullglob

  if [[ ${#files[@]} -eq 0 ]]; then
    echo "• Aucun fichier dans $data_dir — skip"; continue
  fi

  echo "→ $coll (schema: $schema_rel, files: ${#files[@]})"
  for f in "${files[@]}"; do
    run_cli document upsert "$SPACE" "$DB" --schema "$schema_rel" --file "$f"
  done
done

echo "✅ Import dataset terminé."
