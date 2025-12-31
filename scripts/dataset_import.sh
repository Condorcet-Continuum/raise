#!/bin/bash
set -e

# --- CHARGEMENT DU FICHIER .ENV (Si présent) ---
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
    echo "🔧 Configuration chargée depuis .env"
fi

# ==============================================================================
# 1. LOCALISATION DU DATASET
# ==============================================================================

if [ -n "$PATH_RAISE_DATASET" ] && [ -d "$PATH_RAISE_DATASET" ]; then
    DATASET_ROOT="$PATH_RAISE_DATASET"
elif [ -d "../genaptitude_dataset" ]; then
    DATASET_ROOT="$(cd ../genaptitude_dataset && pwd)"
elif [ -d "./genaptitude_dataset" ]; then
    DATASET_ROOT="$(cd ./genaptitude_dataset && pwd)"
else
    echo "❌ Dataset introuvable. Lancez d'abord ./scripts/dataset_init.sh"
    exit 1
fi

echo "📂 Source : $DATASET_ROOT"

# ==============================================================================
# 2. CONFIGURATION DB & CLI
# ==============================================================================

SPACE="un2"
DB="_system"
CLI_BIN="./src-tauri/target/debug/jsondb_cli"

echo "🏗️  Compilation CLI..."
cargo build -p jsondb_cli --quiet

# Création de la DB si nécessaire
$CLI_BIN db create "$SPACE" "$DB" > /dev/null 2>&1 || true

# ==============================================================================
# 3. FONCTION D'IMPORTATION
# ==============================================================================

import_folder() {
    local subpath=$1
    local collection=$2
    local schema=$3
    local full_path="$DATASET_ROOT/arcadia/v1/data/$subpath"
    
    if [ ! -d "$full_path" ]; then
        return # On ignore silencieusement les dossiers vides
    fi

    echo "⬇️  Import $subpath -> $collection"
    
    # Création collection (idempotent)
    $CLI_BIN collection create "$SPACE" "$DB" "$collection" --schema "$schema" > /dev/null 2>&1 || true
    
    # Insertion
    find "$full_path" -name "*.json" -print0 | while IFS= read -r -d '' file; do
        # On capture la sortie d'erreur pour savoir pourquoi ça échoue
        OUTPUT=$($CLI_BIN document insert "$SPACE" "$DB" --schema "$schema" --file "$file" 2>&1)
        EXIT_CODE=$?
        
        if [ $EXIT_CODE -ne 0 ]; then
             echo "   ❌ Erreur $(basename "$file") : $OUTPUT"
        fi
    done
}

echo "🚀 Importation dans le moteur..."

# ==============================================================================
# 4. EXÉCUTION
# ==============================================================================

# DATA
import_folder "exchange-items" "exchange-items" "arcadia/data/exchange-item.schema.json"

# OA
import_folder "oa/actors" "operational-actors" "arcadia/oa/actor.schema.json"
import_folder "oa/activities" "operational-activities" "arcadia/oa/activity.schema.json"
import_folder "oa/capabilities" "operational-capabilities" "arcadia/oa/capability.schema.json"
import_folder "oa/entities" "operational-entities" "arcadia/oa/entity.schema.json"
import_folder "oa/exchanges" "operational-exchanges" "arcadia/oa/activity-exchange.schema.json"

# SA
import_folder "sa/components" "system-components" "arcadia/sa/system-component.schema.json"
import_folder "sa/functions" "system-functions" "arcadia/sa/system-function.schema.json"
import_folder "sa/actors" "system-actors" "arcadia/sa/system-actor.schema.json"
import_folder "sa/capabilities" "system-capabilities" "arcadia/sa/system-capability.schema.json"
import_folder "sa/exchanges" "functional-exchanges-sa" "arcadia/sa/functional-exchange.schema.json"

# LA
import_folder "la/components" "logical-components" "arcadia/la/logical-component.schema.json"
import_folder "la/functions" "logical-functions" "arcadia/la/logical-function.schema.json"
import_folder "la/interfaces" "logical-interfaces" "arcadia/la/logical-interface.schema.json"

# PA
import_folder "pa/components" "physical-components" "arcadia/pa/physical-component.schema.json"
import_folder "pa/functions" "physical-functions" "arcadia/pa/physical-function.schema.json"
import_folder "pa/links" "physical-links" "arcadia/pa/physical-link.schema.json"

# EPBS
import_folder "epbs/configuration-items" "configuration-items" "arcadia/epbs/configuration-item.schema.json"

# TRANSVERSE
import_folder "transverse/property-definitions" "property-definitions" "arcadia/transverse/property-definition.schema.json"
import_folder "transverse/property-values" "property-values" "arcadia/transverse/property-value.schema.json"
import_folder "transverse/requirements" "requirements" "arcadia/transverse/requirement.schema.json"

echo "✨ Base de données synchronisée !"