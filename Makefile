# Makefile - R.A.I.S.E. CI/CD Industrialization
.PHONY: check build clean wasm sbom

# Variables
CARGO := cargo
SBOM_TOOL := cargo-cyclonedx

# 1. Vérification des outils nécessaires
check_deps:
	@command -v $(SBOM_TOOL) >/dev/null 2>&1 || { echo >&2 "ERREUR: $(SBOM_TOOL) est requis mais non installé. Installez-le avec 'cargo install cargo-cyclonedx'."; exit 1; }

# 2. Vérification de la qualité
check: wasm
	@echo "--- 🛡️ Quality Gate ---"
	@$(CARGO) fmt --all -- --check
	@$(CARGO) clippy --workspace -- -D warnings
	@$(CARGO) test --workspace

# 3. Build complet
build: check_deps wasm sbom
	@echo "--- 📦 Building Workspace ---"
	@$(CARGO) build --workspace --release

# 4. Génération du SBOM
sbom:
	@echo "--- 🔍 Generating SBOM ---"
	@$(CARGO) cyclonedx --format json > sbom-rust.cdx.json

# 5. Compilation des modules WASM
wasm:
	@if [ -d "src-wasm" ]; then \
		echo "--- ⚙️ Building WASM Cores ---"; \
		cd src-wasm && ./build.sh; \
	fi

clean:
	@$(CARGO) clean
	@rm -f sbom-rust.cdx.json