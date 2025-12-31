# 0) Variables
APP_DIR=~/raise
OUT_DIR=~/raise_zip
TS=$(date +%Y%m%d_%H%M)
ZIP="$OUT_DIR/raise-$TS.zip"

# 1) Préparer
mkdir -p "$OUT_DIR"
cd "$APP_DIR"

# (facultatif) t’assurer que tout est committé
git status

# 2) Créer le zip (fichiers suivis seulement)
git archive --format=zip --output "$ZIP" HEAD

# 3) Vérifier
unzip -l "$ZIP" | head -n 30
echo "ZIP créé -> $ZIP"
