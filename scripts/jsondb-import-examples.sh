#!/usr/bin/env bash
# scripts/jsondb-import-examples.sh
# Importe des exemples OA (actors, activities, exchanges, capabilities, entities) dans la DB JSON
# via le CLI 'jsondb document upsert'. Utilise PATH_RAISE_DATASET si présent, sinon <repo>/examples/oa_miniproc/data.

set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/jsondb-import-examples.sh <space> <db> [--repo-root=PATH] [--schemas=REL] [--dataset=PATH]

Args:
  <space>   Espace logique (ex: un2)
  <db>      Nom de la DB (ex: _system)

Options:
  --repo-root=PATH   Racine du repo (détection automatique par git sinon)
  --schemas=REL      Chemin relatif des schémas (def: schemas/v1)
  --dataset=PATH     Dossier des exemples (def: $PATH_RAISE_DATASET ou <repo>/examples/oa_miniproc/data)

Exemples:
  scripts/jsondb-import-examples.sh un2 _system
  scripts/jsondb-import-examples.sh un2 _system --repo-root=/work/genaptitude
USAGE
}

if [[ $# < 2 ]]; then usage; exit 1; fi

SPACE="$1"; shift
DB="$1"; shift

REPO_ROOT=""
SCHEMAS_REL="schemas/v1"
DATASET_PATH="${PATH_RAISE_DATASET:-}"

for arg in "$@"; do
  case "$arg" in
    --repo-root=*)
      REPO_ROOT="${arg#*=}"
      ;;
    --schemas=*)
      SCHEMAS_REL="${arg#*=}"
      ;;
    --dataset=*)
      DATASET_PATH="${arg#*=}"
      ;;
    -h|--help)
      usage; exit 0
      ;;
    *)
      echo "Unknown option: $arg" >&2
      usage; exit 1
      ;;
  esac
done

# Repo root
if [[ -z "$REPO_ROOT" ]]; then
  if git_root="$(git rev-parse --show-toplevel 2>/dev/null)"; then
    REPO_ROOT="$git_root"
  else
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
  fi
fi

# Charge .env si présent
if [[ -f "$REPO_ROOT/.env" ]]; then
  echo "🔎 Loading env from: $REPO_ROOT/.env"
  set -a
  # shellcheck disable=SC1090
  source "$REPO_ROOT/.env"
  set +a
fi

# Dataset par défaut si manquant
if [[ -z "${DATASET_PATH:-}" ]]; then
  DATASET_PATH="$REPO_ROOT/examples/oa_miniproc/data"
fi

if [[ ! -d "$DATASET_PATH" ]]; then
  echo "❌ Dataset introuvable: $DATASET_PATH" >&2
  exit 1
fi

SCHEMAS_ROOT="$REPO_ROOT/$SCHEMAS_REL"
echo "➡  Repo root     : $REPO_ROOT"
echo "➡  Schemas root  : $SCHEMAS_ROOT"
echo "➡  Dataset path  : $DATASET_PATH"

run_cli() {
  if command -v jsondb >/dev/null 2>&1; then
    jsondb --repo-root "$REPO_ROOT" "$@"
  else
    ( set -x; cargo run -p jsondb_cli -- --repo-root "$REPO_ROOT" "$@" )
  fi
}

# Mapping collection -> schéma relatif et sous-dossier dataset
declare -A MAP_SCHEMA
declare -A MAP_DIR

MAP_SCHEMA["arcadia_oa_actors"]="arcadia/oa/actor.schema.json"
MAP_DIR["arcadia_oa_actors"]="actors"

MAP_SCHEMA["arcadia_oa_activities"]="arcadia/oa/activity.schema.json"
MAP_DIR["arcadia_oa_activities"]="activities"

MAP_SCHEMA["arcadia_oa_exchanges"]="arcadia/oa/activity-exchange.schema.json"
MAP_DIR["arcadia_oa_exchanges"]="exchanges"

MAP_SCHEMA["arcadia_oa_capabilities"]="arcadia/oa/capability.schema.json"
MAP_DIR["arcadia_oa_capabilities"]="capabilities"

MAP_SCHEMA["arcadia_oa_entities"]="arcadia/oa/entity.schema.json"
MAP_DIR["arcadia_oa_entities"]="entities"

echo "==> Import OA examples (upsert)..."
for coll in "${!MAP_SCHEMA[@]}"; do
  schema_rel="${MAP_SCHEMA[$coll]}"
  dir_rel="${MAP_DIR[$coll]}"
  schema_path="$SCHEMAS_ROOT/$schema_rel"
  data_dir="$DATASET_PATH/$dir_rel"

  if [[ ! -f "$schema_path" ]]; then
    echo "⚠️  Schéma absent, skip: $schema_rel"
    continue
  fi
  if [[ ! -d "$data_dir" ]]; then
    echo "⚠️  Dossier dataset absent, skip: $data_dir"
    continue
  fi

  shopt -s nullglob
  files=( "$data_dir"/*.json )
  shopt -u nullglob

  if [[ ${#files[@]} -eq 0 ]]; then
    echo "• Aucun .json dans $data_dir — skip"
    continue
  fi

  echo "→ Collection $coll  (schema: $schema_rel, files: ${#files[@]})"
  for f in "${files[@]}"; do
    echo "   - $f"
    run_cli document upsert "$SPACE" "$DB" --schema "$schema_rel" --file "$f"
  done
done

echo "✅ Import terminé."
