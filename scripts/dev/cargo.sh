#!/bin/bash

# --- CONFIGURATION DES CHEMINS RELATIFS ---
# On récupère la racine du projet (le dossier où se trouve le script moins deux niveaux)
PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# Le dossier de sortie est maintenant relatif à la structure demandée
OUTPUT_DIR="../raise_dataset/tests"
OUTPUT_FILE="$OUTPUT_DIR/sortie.txt"

# Vérification de l'argument
if [ -z "$1" ]; then
    echo "Usage: bash scripts/dev/cargo <chemin_du_fichier_rust>"
    exit 1
fi

FILE_PATH=$1

# Se placer à la racine du projet pour que les commandes cargo fonctionnent
cd "$PROJECT_ROOT" || exit 1

# Créer le répertoire de sortie s'il n'existe pas
mkdir -p "$OUTPUT_DIR"

# Réinitialiser le fichier de sortie
echo "=== Rapport de build du $(date) ===" > "$OUTPUT_FILE"
echo "Fichier cible : $FILE_PATH" >> "$OUTPUT_FILE"
echo "--------------------------------------" >> "$OUTPUT_FILE"

# Fonction pour exécuter et logger
run_step() {
    local label=$1
    local cmd=$2
    
    echo "Exécution de $label..." | tee -a "$OUTPUT_FILE"
    
    # Exécution de la commande
    eval "$cmd" >> "$OUTPUT_FILE" 2>&1
    local status=$?
    
    # 1. Vérification du code de sortie
    if [ $status -ne 0 ]; then
        echo "❌ ÉCHEC : $label (Erreur rencontrée). Consultez $OUTPUT_FILE"
        exit 1
    fi

    # 2. Vérification des WARNINGS
    if grep -qi "warning:" "$OUTPUT_FILE"; then
        echo "⚠️ ÉCHEC : $label a généré des WARNINGS. Consultez $OUTPUT_FILE"
        exit 1
    fi
    
    echo "✅ $label : OK" | tee -a "$OUTPUT_FILE"
}

# --- ÉTAPES DE VALIDATION ---

# 1. Formatage
run_step "Cargo Fmt" "cargo fmt -- $FILE_PATH"

# 2. Vérification de compilation
run_step "Cargo Check" "cargo check --all-targets --workspace"

# 3. Analyse statique
run_step "Cargo Clippy" "cargo clippy --workspace -- -D warnings"

# 4. Tests Unitaires
FILENAME=$(basename "$FILE_PATH" .rs)
run_step "Cargo Test" "cargo test $FILENAME"

echo "--------------------------------------" >> "$OUTPUT_FILE"
echo "🎉 Toutes les étapes ont réussi pour $FILENAME !" | tee -a "$OUTPUT_FILE"