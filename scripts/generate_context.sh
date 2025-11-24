#!/bin/bash

# --- 1. CONFIGURATION DES CHEMINS ---

# Récupère le dossier où se trouve ce script (/home/zair/genaptitude/scripts)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Le dossier racine du projet est le parent du dossier scripts
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Dossier de destination demandé
DEST_DIR="/home/zair/genaptitude_zip"

# Nom du fichier avec Horodatage (pour ne pas écraser les précédents)
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
OUTPUT_FILE="$DEST_DIR/genaptitude_context_$TIMESTAMP.txt"

# --- 2. PRÉPARATION ---

# Création du dossier de destination s'il n'existe pas
if [ ! -d "$DEST_DIR" ]; then
    echo "📁 Création du dossier $DEST_DIR..."
    mkdir -p "$DEST_DIR"
fi

echo "🚀 Démarrage de l'export pour Gemini..."
echo "📍 Racine du projet : $PROJECT_ROOT"
echo "💾 Fichier de sortie : $OUTPUT_FILE"

# Initialisation du fichier vide
echo "CONTEXTE DU PROJET GENAPTITUDE (Date: $(date))" > "$OUTPUT_FILE"
echo "Stack: Rust / Tauri / WASM / Node.js" >> "$OUTPUT_FILE"
echo "==========================================" >> "$OUTPUT_FILE"

# --- 3. GÉNÉRATION DE L'ARBORESCENCE ---

# On se place à la racine du projet pour que les chemins soient relatifs (plus propre)
cd "$PROJECT_ROOT" || exit

echo "🌳 Génération de l'arborescence..."
echo -e "\n### ARBORESCENCE ###\n" >> "$OUTPUT_FILE"

# Utilise 'tree' s'il est installé, sinon fallback simple
if command -v tree &> /dev/null; then
    # Exclusions spécifiques Tauri/Rust pour l'arbre
    tree -I "target|node_modules|.git|dist|build|pkg|gen|.vscode|.idea|scripts" --noreport >> "$OUTPUT_FILE"
else
    find . -maxdepth 3 -not -path '*/.*' | sed -e "s/[^-][^\/]*\//  |/g" -e "s/|\([^ ]\)/|-\1/" >> "$OUTPUT_FILE"
fi

# --- 4. FUSION DES FICHIERS (CONTENU) ---

echo "📦 Lecture et fusion des fichiers de code..."
echo -e "\n### CONTENU DES FICHIERS ###\n" >> "$OUTPUT_FILE"

# La commande find complexe pour Rust/Tauri
# 1. PRUNE : On interdit purement et simplement l'entrée dans target, node_modules, .git, etc.
# 2. FILTRES : On exclut les binaires, les images, les locks.
find . -type d \( -name "target" -o -name "node_modules" -o -name ".git" -o -name "dist" -o -name "gen" -o -name "pkg" -o -name "scripts" \) -prune -o \
       -type f \
       -not -name "*.wasm" \
       -not -name "*.lock" \
       -not -name "package-lock.json" \
       -not -name "*.png" -not -name "*.jpg" -not -name "*.ico" -not -name "*.svg" \
       -not -name "*.exe" -not -name "*.so" -not -name "*.rlib" -not -name "*.rmeta" \
       -not -name "*.pdf" \
       -not -name ".DS_Store" \
       -print0 | while IFS= read -r -d '' file; do
            
            # Retire le "./" au début du chemin pour la lisibilité
            clean_path="${file#./}"
            
            echo "   -> Ajout : $clean_path"
            
            echo -e "\n==================================================" >> "$OUTPUT_FILE"
            echo "FICHIER : $clean_path" >> "$OUTPUT_FILE"
            echo -e "==================================================\n" >> "$OUTPUT_FILE"
            
            # Ajout du contenu
            cat "$file" >> "$OUTPUT_FILE" 2>/dev/null
done

echo "✅ Terminé ! Fichier généré ici :"
echo "$OUTPUT_FILE"

# Optionnel : Ouvre le dossier de destination
# xdg-open "$DEST_DIR" 2>/dev/null