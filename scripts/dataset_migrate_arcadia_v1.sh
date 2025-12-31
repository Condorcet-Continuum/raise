#!/bin/bash
set -e

# ==============================================================================
# 1. RÉSOLUTION ET CRÉATION DU DOSSIER DATASET
# ==============================================================================

if [ -n "$PATH_RAISE_DATASET" ]; then
    DATASET_ROOT="$PATH_RAISE_DATASET"
    echo "🎯 Cible définie par ENV : $DATASET_ROOT"
elif [ -d "../genaptitude_dataset" ]; then
    DATASET_ROOT="$(cd ../genaptitude_dataset && pwd)"
    echo "📂 Dossier existant trouvé (Frère) : $DATASET_ROOT"
else
    DATASET_ROOT="$(pwd)/../genaptitude_dataset"
    echo "✨ Création d'un nouveau dataset dans : $DATASET_ROOT"
fi

mkdir -p "$DATASET_ROOT"

# ==============================================================================
# 2. FONCTION DE SEEDING
# ==============================================================================

seed_file() {
    local rel_path=$1
    local content=$2
    local full_path="$DATASET_ROOT/arcadia/v1/data/$rel_path"
    local dir_path=$(dirname "$full_path")

    mkdir -p "$dir_path"

    if [ ! -f "$full_path" ]; then
        echo "$content" > "$full_path"
        echo "   ✅ Créé : $rel_path"
    else
        echo "   ⏭️  Existe déjà : $rel_path"
    fi
}

echo "🚀 Initialisation du contenu du Dataset..."

# ==============================================================================
# 3. DONNÉES
# ==============================================================================

# DATA
seed_file "exchange-items/position_gps.json" '{
  "id": "urn:uuid:ei-position-gps",
  "name": { "fr": "Position GPS", "en": "GPS Position" },
  "exchangeMechanism": "Flow"
}'

# OA
seed_file "oa/actors/client.json" '{
  "id": "urn:uuid:oa-actor-client",
  "name": "Client Final",
  "isHuman": true,
  "description": { "fr": "Personne recevant le colis." }
}'
seed_file "oa/activities/commander.json" '{
  "id": "urn:uuid:oa-activity-commander",
  "name": "Commander un colis",
  "allocatedTo": ["urn:uuid:oa-actor-client"]
}'
seed_file "oa/capabilities/livraison_rapide.json" '{
  "id": "urn:uuid:oa-cap-livraison",
  "name": "Livraison Express",
  "involvedActivities": ["urn:uuid:oa-activity-commander"]
}'

# SA
seed_file "sa/components/drone_system.json" '{
  "id": "urn:uuid:sys-drone",
  "name": "Système Drone",
  "allocatedFunctions": ["urn:uuid:sf-voler"]
}'
seed_file "sa/functions/voler.json" '{
  "id": "urn:uuid:sf-voler",
  "name": "Voler vers destination",
  "allocatedTo": ["urn:uuid:sys-drone"],
  "realizedActivities": ["urn:uuid:oa-activity-commander"],
  "inputs": [],
  "outputs": []
}'

# LA
seed_file "la/components/nav_unit.json" '{
  "id": "urn:uuid:lc-navigation",
  "name": "Unité de Navigation",
  "realizedSystemComponents": ["urn:uuid:sys-drone"]
}'

# PA
seed_file "pa/components/motor.json" '{
  "id": "urn:uuid:pc-motor",
  "name": "Moteur Brushless A2212",
  "nature": "Node",
  "realizedLogicalComponents": ["urn:uuid:lc-navigation"],
  "propertyValues": ["urn:uuid:pv-motor-mass"]
}'

# TRANSVERSE
seed_file "transverse/property-definitions/mass_budget.json" '{
  "id": "urn:uuid:prop-def-mass",
  "name": "Mass Budget",
  "domain": "Mechanical",
  "appliesTo": ["PhysicalComponent"],
  "fields": [
    { "name": "weight_kg", "type": "Float", "unit": "kg" }
  ]
}'

seed_file "transverse/property-values/motor_mass.json" '{
  "id": "urn:uuid:pv-motor-mass",
  "definitionId": "urn:uuid:prop-def-mass",
  "appliedTo": "urn:uuid:pc-motor",
  "values": { "weight_kg": 0.45 }
}'

echo "✨ Initialisation terminée dans : $DATASET_ROOT"