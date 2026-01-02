#!/bin/bash

# ==============================================================================
# GENERATE CONTEXT FOR LLM
# ==============================================================================
# Ce script compile l'arborescence et le contenu des fichiers essentiels du projet
# dans un seul fichier texte, optimisé pour être copié-collé dans un prompt IA.

# --- CONFIGURATION ---
OUTPUT_DIR="$HOME/raise_zip"
OUTPUT_FILE="$OUTPUT_DIR/raise_context.txt"

# Dossiers à ignorer (Regex pour la commande tree)
IGNORE_PATTERN="target|node_modules|.git|dist|wasm-modules|build|venv|.fastembed_cache|raise_dataset"

# --- DÉMARRAGE ---
echo "🚀 Démarrage de la génération du contexte pour LLM..."
echo "📂 Racine du projet : $(pwd)"

mkdir -p "$OUTPUT_DIR"

# En-tête du fichier
echo "==============================================================================" > "$OUTPUT_FILE"
echo " PROJECT: RAISE" >> "$OUTPUT_FILE"
echo " GENERATED ON: $(date)" >> "$OUTPUT_FILE"
echo " CONTENT: Tree + Configs + Docs + Source Code (Rust/React)" >> "$OUTPUT_FILE"
echo "==============================================================================" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# ------------------------------------------------------------------------------
# 1. ARBORESCENCE (TREE)
# ------------------------------------------------------------------------------
echo "🌳 Génération de l'arborescence..."
echo "### SECTION 1: PROJECT STRUCTURE ###" >> "$OUTPUT_FILE"
echo '```' >> "$OUTPUT_FILE"
if command -v tree &> /dev/null; then
    tree -I "$IGNORE_PATTERN" --prune >> "$OUTPUT_FILE"
else
    # Fallback si 'tree' n'est pas installé
    find . -maxdepth 4 -not -path '*/.*' | grep -vE "node_modules|target|dist" >> "$OUTPUT_FILE"
fi
echo '```' >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# Fonction utilitaire pour ajouter des fichiers au contexte
add_files() {
    local SECTION_TITLE="$1"
    local SEARCH_PATH="$2"
    local EXTENSIONS="$3" # ex: "-name *.rs -o -name *.toml"
    
    echo "📄 Ajout section : $SECTION_TITLE"
    echo "### SECTION: $SECTION_TITLE ###" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"

    # Construction de la commande find avec exclusions
    # On utilise -prune pour ne même pas descendre dans les dossiers lourds
    find "$SEARCH_PATH" \
        -type d \( -name node_modules -o -name target -o -name .git -o -name venv -o -name .fastembed_cache -o -name dist \) -prune \
        -o -type f \( $EXTENSIONS \) -print | sort | while read -r file; do
        
        # On ignore les fichiers de lock volumineux et les datasets
        if [[ "$file" == *"package-lock.json"* ]] || [[ "$file" == *"Cargo.lock"* ]] || [[ "$file" == *".jsonl"* ]]; then
            continue
        fi

        echo "  -> $file"
        echo "------------------------------------------------------------------------------" >> "$OUTPUT_FILE"
        echo "FILE PATH: $file" >> "$OUTPUT_FILE"
        echo "------------------------------------------------------------------------------" >> "$OUTPUT_FILE"
        echo '```' >> "$OUTPUT_FILE"
        cat "$file" >> "$OUTPUT_FILE"
        echo "" >> "$OUTPUT_FILE"
        echo '```' >> "$OUTPUT_FILE"
        echo "" >> "$OUTPUT_FILE"
    done
    echo "" >> "$OUTPUT_FILE"
}

# ------------------------------------------------------------------------------
# 2. FICHIERS DE CONFIGURATION CRITIQUES
# ------------------------------------------------------------------------------
# On cherche à la racine et dans src-tauri
add_files "CONFIGURATION FILES" "." "-name Cargo.toml -o -name package.json -o -name tauri.conf.json -o -name .env.example"

# ------------------------------------------------------------------------------
# 3. DOCUMENTATION (Markdown)
# ------------------------------------------------------------------------------
add_files "DOCUMENTATION" "." "-name *.md"

# ------------------------------------------------------------------------------
# 4. BACKEND RUST (src-tauri)
# ------------------------------------------------------------------------------
# On se concentre sur le code source Rust
add_files "BACKEND SOURCE (RUST)" "src-tauri/src" "-name *.rs"

# ------------------------------------------------------------------------------
# 5. FRONTEND REACT (src)
# ------------------------------------------------------------------------------
# On récupère les composants et la logique (TS/TSX), mais on limite aux sources
add_files "FRONTEND SOURCE (REACT)" "src" "-name *.tsx -o -name *.ts"

# ------------------------------------------------------------------------------
# 6. SCHÉMAS & DEFINITIONS (JSON)
# ------------------------------------------------------------------------------
# Uniquement les JSON de configuration/schéma, pas les données brutes
add_files "SCHEMAS & DEFINITIONS" "src-tauri" "-name *.json"

echo "==============================================================================" >> "$OUTPUT_FILE"
echo "END OF CONTEXT" >> "$OUTPUT_FILE"

echo ""
echo "✅ Terminé ! Le fichier de contexte complet est prêt :"
echo "👉 $OUTPUT_FILE"
# Affiche la taille pour info
du -h "$OUTPUT_FILE"