#!/bin/bash
set -e

# Définition des chemins absolus
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$SCRIPT_DIR/.."
WASM_MODULES_DIR="$ROOT_DIR/wasm-modules"

# ✅ NOUVEAU : Le dossier de sortie commun à tout le workspace
WORKSPACE_TARGET_DIR="$SCRIPT_DIR/target/wasm32-unknown-unknown/release"

echo "🏭 GenAptitude Factory : Démarrage de la compilation..."
echo "======================================================"

# On cherche tous les sous-dossiers dans src-wasm/blocks/
for BLOCK_PATH in "$SCRIPT_DIR/blocks"/*; do
    if [ -d "$BLOCK_PATH" ]; then
        PLUGIN_NAME=$(basename "$BLOCK_PATH")
        
        echo "🔧 Traitement du bloc : $PLUGIN_NAME"

        # 1. Tests Unitaires
        echo "   🧪 Exécution des tests..."
        (cd "$BLOCK_PATH" && cargo test --quiet)

        # 2. Compilation WASM
        # Note : Cargo détecte qu'il est dans un workspace et va écrire dans src-wasm/target
        echo "   ⚙️  Compilation WASM..."
        (cd "$BLOCK_PATH" && cargo build --release --target wasm32-unknown-unknown --quiet)

        # 3. Récupération & Déploiement
        # Rust remplace les tirets par des underscores
        RUST_FILE_NAME="${PLUGIN_NAME//-/_}.wasm"
        
        # 👇 CORRECTION ICI : On cherche dans le dossier cible commun du Workspace
        SOURCE_WASM="$WORKSPACE_TARGET_DIR/$RUST_FILE_NAME"
        
        # Destination : wasm-modules/<nom_du_plugin>/
        DEST_DIR="$WASM_MODULES_DIR/$PLUGIN_NAME"
        mkdir -p "$DEST_DIR"

        if [ -f "$SOURCE_WASM" ]; then
            cp "$SOURCE_WASM" "$DEST_DIR/$PLUGIN_NAME.wasm"
            echo "   ✅ Succès : Installé dans wasm-modules/$PLUGIN_NAME/$PLUGIN_NAME.wasm"
        else
            echo "   ❌ ERREUR : Le fichier $SOURCE_WASM est introuvable."
            echo "      (Vérifiez que le nom du package dans Cargo.toml correspond bien au nom du dossier)"
            exit 1
        fi
        echo "------------------------------------------------------"
    fi
done

echo "🎉 Tout est terminé avec succès."