#!/usr/bin/env bash
# scripts/jsondb-quickstart.sh
# Quick bootstrap for GenAptitude JSON-DB + OA collections.
# - Loads .env from the repo root (to get PATH_RAISE_DOMAIN)
# - Uses `jsondb` if installed, otherwise falls back to `cargo run -p jsondb_cli`.

set -euo pipefail

# ----------- Usage -----------
usage() {
  cat <<'USAGE'
Usage:
  scripts/jsondb-quickstart.sh <space> <db> [--repo-root=PATH] [--schemas=REL_PATH] [--with-diagram] [--with-process]

Args:
  <space>      Logical space name (e.g., un2)
  <db>         Database name     (e.g., _system)

Options:
  --repo-root=PATH   Path to the GenAptitude repo root (default: auto-detect via git, else script/..)
  --schemas=REL      Schemas folder relative to repo root (default: schemas/v1)
  --with-diagram     Also create collection for class-diagrams (Arcadia OA)
  --with-process     Also create collection for OA mini-processes

Environment:
  PATH_RAISE_DOMAIN (preferably set in the repo's .env at root)
    Example .env:
      PATH_RAISE_DOMAIN="$HOME/genaptitude_domain"

Examples:
  scripts/jsondb-quickstart.sh un2 _system
  scripts/jsondb-quickstart.sh un2 _system --with-diagram --with-process
  scripts/jsondb-quickstart.sh un2 _system --repo-root=/work/genaptitude --schemas=schemas/v1
USAGE
}

# ----------- Parse args -----------
if [[ $# -lt 2 ]]; then
  usage; exit 1
fi

SPACE="$1"; shift
DB="$1"; shift

REPO_ROOT=""
SCHEMAS_REL="schemas/v1"
WITH_DIAGRAM=0
WITH_PROCESS=0

for arg in "$@"; do
  case "$arg" in
    --repo-root=*)
      REPO_ROOT="${arg#*=}"
      ;;
    --schemas=*)
      SCHEMAS_REL="${arg#*=}"
      ;;
    --with-diagram)
      WITH_DIAGRAM=1
      ;;
    --with-process)
      WITH_PROCESS=1
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

# ----------- Resolve repo root -----------
if [[ -z "$REPO_ROOT" ]]; then
  if git_root="$(git rev-parse --show-toplevel 2>/dev/null)"; then
    REPO_ROOT="$git_root"
  else
    # fallback: script dir/..
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
  fi
fi

# ----------- Load .env from repo root (if present) -----------
if [[ -f "$REPO_ROOT/.env" ]]; then
  echo "🔎 Loading env from: $REPO_ROOT/.env"
  set -a
  # shellcheck disable=SC1090
  source "$REPO_ROOT/.env"
  set +a
fi

# ----------- Preconditions -----------
if [[ -z "${PATH_RAISE_DOMAIN:-}" ]]; then
  echo "❌ ENV PATH_RAISE_DOMAIN manquant." >&2
  echo "   Ajoute-le dans $REPO_ROOT/.env, par ex.:" >&2
  echo '     PATH_RAISE_DOMAIN="$HOME/genaptitude_domain"' >&2
  exit 1
fi

SCHEMAS_ROOT="$REPO_ROOT/$SCHEMAS_REL"

echo "➡  Repo root          : $REPO_ROOT"
echo "➡  Schemas root       : $SCHEMAS_ROOT"
echo "➡  Domain root (ENV)  : $PATH_RAISE_DOMAIN"
echo "➡  Target space/db    : $SPACE / $DB"

# ----------- CLI runner -----------
run_cli() {
  if command -v jsondb >/dev/null 2>&1; then
    jsondb --repo-root "$REPO_ROOT" "$@"
  else
    ( set -x; cargo run -p jsondb_cli -- --repo-root "$REPO_ROOT" "$@" )
  fi
}

# ----------- Create or open DB -----------
echo "==> Checking/creating DB..."
set +e
run_cli db create "$SPACE" "$DB"
CREATE_RC=$?
set -e
if [[ $CREATE_RC -ne 0 ]]; then
  echo "   (DB may already exist) Opening instead..."
else
  echo "✅ DB créée: $SPACE/$DB"
fi
run_cli db open "$SPACE" "$DB"

# ----------- Collections to create -----------
declare -A COLL
COLL["arcadia_oa_actors"]="arcadia/oa/actor.schema.json"
COLL["arcadia_oa_activities"]="arcadia/oa/activity.schema.json"
COLL["arcadia_oa_exchanges"]="arcadia/oa/activity-exchange.schema.json"
COLL["arcadia_oa_capabilities"]="arcadia/oa/capability.schema.json"
COLL["arcadia_oa_entities"]="arcadia/oa/entity.schema.json"

if [[ $WITH_DIAGRAM -eq 1 ]]; then
  COLL["arcadia_class_diagrams"]="arcadia/metamodel/class-diagram.schema.json"
fi
if [[ $WITH_PROCESS -eq 1 ]]; then
  COLL["arcadia_oa_processes"]="arcadia/oa/process.schema.json"
fi

echo "==> Creating OA collections (if schemas exist) ..."
for name in "${!COLL[@]}"; do
  schema_rel="${COLL[$name]}"
  schema_path="$SCHEMAS_ROOT/$schema_rel"
  if [[ -f "$schema_path" ]]; then
    echo " -> $name  (schema: $schema_rel)"
    set +e
    run_cli collection create "$SPACE" "$DB" "$name" --schema "$schema_rel"
    rc=$?
    set -e
    if [[ $rc -ne 0 ]]; then
      echo "    ⚠️  Skip or already exists (rc=$rc)"
    fi
  else
    echo "    ⚠️  Schema not found, skipping: $schema_rel"
  fi
done

echo "✅ Done. Collections attempted: ${#COLL[@]}"
echo "   Tips:"
echo "     - Les données OA s'insèrent via l'app Tauri ou un outil d'import dédié."
echo "     - Pour inclure diagramme/process: --with-diagram --with-process"
