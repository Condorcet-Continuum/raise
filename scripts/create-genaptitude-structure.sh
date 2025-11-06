#!/bin/bash

################################################################################
# GenAptitude - Script de Création de Structure
# Ubuntu 24.04
# Architecture: Tauri + WASM + TypeScript/JavaScript
################################################################################

set -e  # Arrêt en cas d'erreur

# Couleurs pour l'affichage
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Fonction pour afficher les messages
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

print_section() {
    echo -e "\n${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${YELLOW}  $1${NC}"
    echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"
}

# Nom du projet
PROJECT_NAME="genaptitude"

# Vérifier si le répertoire existe déjà
if [ -d "$PROJECT_NAME" ]; then
    echo -e "${YELLOW}⚠ Le répertoire '$PROJECT_NAME' existe déjà.${NC}"
    read -p "Voulez-vous le supprimer et recréer la structure ? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf "$PROJECT_NAME"
        print_success "Répertoire existant supprimé"
    else
        echo "Opération annulée."
        exit 0
    fi
fi

print_section "Création de la structure GenAptitude"

# Créer le répertoire racine
mkdir -p "$PROJECT_NAME"
cd "$PROJECT_NAME"
print_success "Répertoire racine créé: $PROJECT_NAME"

################################################################################
# DOCUMENTATION
################################################################################
print_section "Documentation"

mkdir -p docs/{architecture/{adr,domain-models},user-guides/tutorials}
touch docs/architecture/{functional-architecture.md,technical-architecture.md}
touch docs/architecture/domain-models/{software-domain.md,system-domain.md,hardware-domain.md}
touch docs/user-guides/{getting-started.md,ai-interface-guide.md}

print_success "Structure de documentation créée"

################################################################################
# SOURCE FRONTEND (React + TypeScript)
################################################################################
print_section "Frontend - React + TypeScript"

mkdir -p src/{components,features,services,hooks,store,utils,types,styles,assets}

# Composants UI IA Native
mkdir -p src/components/{ai-chat,model-viewer,code-editor,workflow-designer,diagram-editor,shared}
touch src/components/ai-chat/{ChatInterface.tsx,MessageBubble.tsx,InputBar.tsx,SuggestionPanel.tsx,IntentClassifier.tsx,ContextDisplay.tsx}
touch src/components/model-viewer/{CapellaViewer.tsx,DiagramRenderer.tsx,ModelNavigator.tsx,ArcadiaLayerView.tsx,ElementInspector.tsx}
touch src/components/code-editor/{CodeEditor.tsx,SyntaxHighlighter.tsx,CodeCompletion.tsx,LivePreview.tsx}
touch src/components/workflow-designer/{WorkflowCanvas.tsx,NodeLibrary.tsx,ConnectionManager.tsx,ExecutionMonitor.tsx}
touch src/components/diagram-editor/{DiagramCanvas.tsx,ShapeLibrary.tsx,ConnectionTool.tsx,LayoutEngine.tsx}
touch src/components/shared/{Button.tsx,Card.tsx,Modal.tsx,Tabs.tsx,SplitPane.tsx,TreeView.tsx}

# Features par domaine
mkdir -p src/features/{software-engineering,system-engineering,hardware-engineering,project-management}
touch src/features/software-engineering/{SoftwareWorkspace.tsx,ComponentDesigner.tsx,ArchitectureBuilder.tsx,CodeGenerator.tsx,DependencyGraph.tsx,PatternSelector.tsx}
touch src/features/system-engineering/{SystemWorkspace.tsx,RequirementManager.tsx,ArchitectureEditor.tsx,FunctionalChainEditor.tsx,ModeStateEditor.tsx,ComplianceChecker.tsx}
touch src/features/hardware-engineering/{HardwareWorkspace.tsx,SchematicEditor.tsx,PCBDesigner.tsx,HDLEditor.tsx,SignalAnalyzer.tsx,ComponentLibrary.tsx}
touch src/features/project-management/{ProjectDashboard.tsx,FileExplorer.tsx,VersionControl.tsx,ExportManager.tsx}

# Services
touch src/services/{tauri-commands.ts,ai-service.ts,model-service.ts,code-service.ts,file-service.ts,wasm-bridge.ts}

# Hooks
touch src/hooks/{useAIChat.ts,useModelState.ts,useCodeGeneration.ts,useFileSystem.ts,useTauriEvent.ts}

# Store
touch src/store/{index.ts,ai-store.ts,model-store.ts,project-store.ts,ui-store.ts,settings-store.ts}

# Utils
touch src/utils/{formatters.ts,validators.ts,parsers.ts,converters.ts,helpers.ts}

# Types
touch src/types/{ai.types.ts,model.types.ts,domain.types.ts,arcadia.types.ts,tauri.types.ts}

# Styles
mkdir -p src/styles/themes
touch src/styles/{globals.css,variables.css}
touch src/styles/themes/{light.css,dark.css}

# Assets
mkdir -p src/assets/{icons,images,fonts}

# Fichiers principaux
touch src/{main.tsx,App.tsx}

print_success "Structure frontend créée"

################################################################################
# BACKEND RUST (Tauri)
################################################################################
print_section "Backend - Rust (Tauri)"

mkdir -p src-tauri/src/{commands,ai,model_engine,code_generator,traceability,workflow_engine,storage,utils,plugins}
mkdir -p src-tauri/icons

# Commands
touch src-tauri/src/commands/{mod.rs,ai_commands.rs,model_commands.rs,code_commands.rs,file_commands.rs,project_commands.rs}

# AI Module
mkdir -p src-tauri/src/ai/{agents,llm,context,nlp}
touch src-tauri/src/ai/{mod.rs,orchestrator.rs}
touch src-tauri/src/ai/agents/{mod.rs,intent_classifier.rs,software_agent.rs,system_agent.rs,hardware_agent.rs}
touch src-tauri/src/ai/llm/{mod.rs,client.rs,prompts.rs,response_parser.rs}
touch src-tauri/src/ai/context/{mod.rs,conversation_manager.rs,memory_store.rs}
touch src-tauri/src/ai/nlp/{mod.rs,parser.rs,entity_extractor.rs}

# Model Engine
mkdir -p src-tauri/src/model_engine/{arcadia,capella,transformers,validators}
touch src-tauri/src/model_engine/{mod.rs}
touch src-tauri/src/model_engine/arcadia/{mod.rs,operational_analysis.rs,system_analysis.rs,logical_architecture.rs,physical_architecture.rs,epbs.rs}
touch src-tauri/src/model_engine/capella/{mod.rs,model_reader.rs,model_writer.rs,xmi_parser.rs,diagram_generator.rs}
touch src-tauri/src/model_engine/transformers/{mod.rs,dialogue_to_model.rs,software_transformer.rs,system_transformer.rs,hardware_transformer.rs}
touch src-tauri/src/model_engine/validators/{mod.rs,consistency_checker.rs,compliance_validator.rs}

# Code Generator
mkdir -p src-tauri/src/code_generator/{generators,templates,analyzers}
touch src-tauri/src/code_generator/{mod.rs}
touch src-tauri/src/code_generator/generators/{mod.rs,typescript_gen.rs,rust_gen.rs,cpp_gen.rs,vhdl_gen.rs,verilog_gen.rs}
touch src-tauri/src/code_generator/templates/{mod.rs,template_engine.rs}
touch src-tauri/src/code_generator/analyzers/{mod.rs,dependency_analyzer.rs}

# Traceability
mkdir -p src-tauri/src/traceability/{compliance,reporting}
touch src-tauri/src/traceability/{mod.rs,tracer.rs,change_tracker.rs,impact_analyzer.rs}
touch src-tauri/src/traceability/compliance/{mod.rs,iso_26262.rs,iec_61508.rs,do_178c.rs}
touch src-tauri/src/traceability/reporting/{mod.rs,trace_matrix.rs,audit_report.rs}

# Workflow Engine
touch src-tauri/src/workflow_engine/{mod.rs,executor.rs,scheduler.rs,state_machine.rs}

# Storage
mkdir -p src-tauri/src/storage/db
touch src-tauri/src/storage/{mod.rs,project_manager.rs,file_manager.rs,cache_manager.rs}
touch src-tauri/src/storage/db/{mod.rs,sqlite_store.rs,migrations.rs}

# Utils
touch src-tauri/src/utils/{mod.rs,error.rs,logger.rs,config.rs}

# Plugins
touch src-tauri/src/plugins/{mod.rs,filesystem_extended.rs}

# Fichiers principaux Tauri
touch src-tauri/src/{main.rs,lib.rs}
touch src-tauri/{Cargo.toml,tauri.conf.json,build.rs}

print_success "Structure backend Rust créée"

################################################################################
# WASM (WebAssembly)
################################################################################
print_section "WebAssembly - Rust → WASM"

mkdir -p src-wasm/src/{parsing,computation,serialization,analysis}
mkdir -p src-wasm/pkg

# Parsing
touch src-wasm/src/parsing/{mod.rs,xmi_parser.rs,capella_parser.rs,sysml_parser.rs}

# Computation
touch src-wasm/src/computation/{mod.rs,graph_algorithms.rs,dependency_resolution.rs,layout_engine.rs,optimization.rs}

# Serialization
touch src-wasm/src/serialization/{mod.rs,model_serializer.rs,binary_format.rs}

# Analysis
touch src-wasm/src/analysis/{mod.rs,impact_analysis.rs,traceability_matrix.rs,consistency_check.rs}

touch src-wasm/src/lib.rs
touch src-wasm/{Cargo.toml,build.sh}

print_success "Structure WASM créée"

################################################################################
# DOMAIN MODELS
################################################################################
print_section "Modèles Métier par Domaine"

# Software Engineering
mkdir -p domain-models/software/{arcadia-models/{operational-analysis,system-analysis,logical-architecture,physical-architecture},patterns,templates}
touch domain-models/software/patterns/{architectural-patterns.json,design-patterns.json,integration-patterns.json}
touch domain-models/software/templates/{microservices-template.json,monolithic-template.json,serverless-template.json}

# System Engineering
mkdir -p domain-models/system/{arcadia-models/{operational-analysis,system-analysis,logical-architecture,physical-architecture},sysml-templates,standards/{iso-26262,iec-61508,do-178c}}

# Hardware Engineering
mkdir -p domain-models/hardware/{arcadia-models/{operational-analysis,system-analysis,logical-architecture,physical-architecture},hdl-templates/{vhdl,verilog},specifications/{electrical-specs,mechanical-specs,thermal-specs}}

print_success "Modèles métier créés"

################################################################################
# AI ASSETS
################################################################################
print_section "Assets IA"

mkdir -p ai-assets/{prompts/{system-prompts,few-shot-examples},embeddings/{domain-vocabulary,patterns}}
touch ai-assets/prompts/system-prompts/{software-engineer.txt,system-engineer.txt,hardware-engineer.txt}
touch ai-assets/prompts/few-shot-examples/{software-examples.json,system-examples.json,hardware-examples.json}
touch ai-assets/embeddings/domain-vocabulary/{software-vocab.json,system-vocab.json,hardware-vocab.json}
touch ai-assets/embeddings/patterns/common-patterns.json

print_success "Assets IA créés"

################################################################################
# TESTS
################################################################################
print_section "Tests"

mkdir -p tests/{unit/{frontend,rust,wasm},integration/{tauri-commands,model-generation,code-generation},e2e/scenarios,performance/wasm-benchmarks}
touch tests/e2e/scenarios/{software-creation.spec.ts,system-modeling.spec.ts,hardware-design.spec.ts}

print_success "Structure de tests créée"

################################################################################
# SCRIPTS
################################################################################
print_section "Scripts Utilitaires"

mkdir -p scripts/{setup,build,dev}
touch scripts/setup/{install-deps.sh,setup-dev.sh}
touch scripts/build/{build-wasm.sh,build-tauri.sh,bundle-app.sh}
touch scripts/dev/{dev-server.sh,watch-wasm.sh}

# Rendre les scripts exécutables
chmod +x scripts/setup/*.sh
chmod +x scripts/build/*.sh
chmod +x scripts/dev/*.sh

print_success "Scripts utilitaires créés"

################################################################################
# CONFIGURATION
################################################################################
print_section "Configuration"

mkdir -p config/defaults
touch config/{ai-config.json,model-config.json,editor-config.json}
touch config/defaults/{project-template.json,user-preferences.json}

print_success "Fichiers de configuration créés"

################################################################################
# EXAMPLES
################################################################################
print_section "Exemples & Tutoriels"

mkdir -p examples/{software-projects/{microservices-example,web-app-example,desktop-app-example},system-projects/{automotive-system,iot-device,industrial-control},hardware-projects/{embedded-board,sensor-module,power-circuit}}

print_success "Exemples créés"

################################################################################
# FICHIERS RACINE & CONFIGURATION IDE
################################################################################
print_section "Fichiers Racine & IDE"

# GitHub
mkdir -p .github/{workflows,ISSUE_TEMPLATE}
touch .github/workflows/{build.yml,test.yml,release.yml}

# VSCode
mkdir -p .vscode
touch .vscode/{settings.json,extensions.json,launch.json}

# Fichiers de configuration racine
cat > package.json << 'EOF'
{
  "name": "genaptitude",
  "version": "0.1.0",
  "description": "Plateforme d'Ingénierie Multi-Domaines à Interface IA Native",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "tauri": "tauri",
    "tauri:dev": "tauri dev",
    "tauri:build": "tauri build",
    "wasm:build": "cd src-wasm && ./build.sh",
    "test": "vitest",
    "lint": "eslint . --ext ts,tsx --report-unused-disable-directives --max-warnings 0"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "zustand": "^4.5.0",
    "react-flow-renderer": "^10.3.17",
    "@monaco-editor/react": "^4.6.0",
    "framer-motion": "^11.0.0"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0",
    "@types/react": "^18.3.1",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.2.1",
    "typescript": "^5.4.5",
    "vite": "^5.2.0",
    "vitest": "^1.5.0",
    "eslint": "^8.57.0",
    "autoprefixer": "^10.4.19",
    "postcss": "^8.4.38",
    "tailwindcss": "^3.4.3"
  }
}
EOF

cat > tsconfig.json << 'EOF'
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"]
    }
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
EOF

cat > vite.config.ts << 'EOF'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: ['es2021', 'chrome100', 'safari13'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
})
EOF

cat > tailwind.config.js << 'EOF'
/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'software': '#0066CC',
        'system': '#009966',
        'hardware': '#CC6600',
        'ai': '#9933CC',
      },
    },
  },
  plugins: [],
}
EOF

cat > .gitignore << 'EOF'
# Dependencies
node_modules/
target/

# Build outputs
dist/
dist-ssr/
src-tauri/target/
src-wasm/pkg/
src-wasm/target/

# Logs
*.log
npm-debug.log*
yarn-debug.log*
yarn-error.log*
pnpm-debug.log*
lerna-debug.log*

# Editor directories and files
.vscode/*
!.vscode/extensions.json
!.vscode/settings.json
!.vscode/launch.json
.idea
.DS_Store
*.suo
*.ntvs*
*.njsproj
*.sln
*.sw?

# Environment
.env
.env.local
.env.*.local

# OS
Thumbs.db
.DS_Store

# Tauri
src-tauri/target
src-tauri/Cargo.lock
EOF

touch .env.example

cat > README.md << 'EOF'
# GenAptitude

**Plateforme d'Ingénierie Multi-Domaines à Interface IA Native**

GenAptitude révolutionne la conception et l'implémentation de systèmes complexes en permettant aux ingénieurs de créer des architectures logicielles, système et hardware par dialogue naturel, avec garantie de conformité, traçabilité et explicabilité.

## 🎯 Proposition de Valeur

Interface **IA Native multimodale** qui traduit automatiquement l'intention ingénieur en modélisation formelle Arcadia/Capella pour **trois domaines d'ingénierie** :

- 🔵 **Software Engineering** - Architecture logicielle, génération de code
- 🟢 **System Engineering** - Modélisation système, conformité normative
- 🟠 **Hardware Engineering** - Conception électronique, HDL

## 🚀 Stack Technique

- **Desktop**: Tauri 2.0+ (Rust + React)
- **Frontend**: React 18, TypeScript, TailwindCSS
- **Performance**: WebAssembly (Rust)
- **IA**: Claude API / GPT-4 API
- **Modélisation**: Arcadia/Capella (XMI/XML)

## 📦 Installation

```bash
# Installer les dépendances
npm install

# Développement
npm run tauri:dev

# Build production
npm run tauri:build
```

## 🏗️ Architecture

Voir `docs/architecture/` pour la documentation complète.

## 📝 Licence

Voir fichier `LICENSE`
EOF

cat > CONTRIBUTING.md << 'EOF'
# Guide de Contribution

Merci de votre intérêt pour contribuer à GenAptitude !

## Prérequis

- Node.js 18+
- Rust 1.75+
- pnpm / npm

## Workflow de Développement

1. Fork le projet
2. Créer une branche (`git checkout -b feature/AmazingFeature`)
3. Commit les changements (`git commit -m 'Add AmazingFeature'`)
4. Push vers la branche (`git push origin feature/AmazingFeature`)
5. Ouvrir une Pull Request

## Standards de Code

- TypeScript strict mode
- ESLint + Prettier
- Tests unitaires requis
- Documentation des fonctions publiques

## Tests

```bash
npm run test
```
EOF

touch LICENSE

print_success "Fichiers racine créés"

################################################################################
# CRÉATION DE README DANS LES DOSSIERS PRINCIPAUX
################################################################################
print_section "Création des README.md"

cat > docs/README.md << 'EOF'
# Documentation GenAptitude

- `architecture/` - Documentation d'architecture (ADR, modèles de domaine)
- `user-guides/` - Guides utilisateur et tutoriels
EOF

cat > src/README.md << 'EOF'
# Frontend Source (React + TypeScript)

- `components/` - Composants UI réutilisables
- `features/` - Fonctionnalités par domaine (Software/System/Hardware)
- `services/` - Services d'intégration (Tauri, IA, etc.)
- `hooks/` - React hooks personnalisés
- `store/` - State management (Zustand)
EOF

cat > src-tauri/README.md << 'EOF'
# Backend Rust (Tauri)

Code Rust pour :
- Commandes Tauri exposées au frontend
- Orchestration IA multi-agents
- Moteur de modélisation Arcadia/Capella
- Génération de code
- Traçabilité et conformité
EOF

cat > src-wasm/README.md << 'EOF'
# Modules WebAssembly

Code Rust compilé en WASM pour performances critiques :
- Parsing XMI/XML
- Algorithmes de graphe
- Calculs de layout
- Analyses de modèles
EOF

cat > domain-models/README.md << 'EOF'
# Modèles Métier par Domaine

- `software/` - Templates et patterns pour l'ingénierie logicielle
- `system/` - Modèles et standards pour l'ingénierie système
- `hardware/` - Templates HDL et spécifications matérielles
EOF

print_success "README.md créés"

################################################################################
# RÉSUMÉ
################################################################################
print_section "CRÉATION TERMINÉE !"

echo -e "${GREEN}Structure GenAptitude créée avec succès !${NC}\n"

echo "📁 Structure créée:"
echo "   - Documentation complète"
echo "   - Frontend React + TypeScript"
echo "   - Backend Rust (Tauri)"
echo "   - Modules WebAssembly"
echo "   - Modèles métier (Software/System/Hardware)"
echo "   - Assets IA (prompts, embeddings)"
echo "   - Tests (unit/integration/e2e)"
echo "   - Scripts utilitaires"
echo "   - Configuration IDE"
echo ""

echo "🚀 Prochaines étapes:"
echo "   1. cd $PROJECT_NAME"
echo "   2. npm install"
echo "   3. cd src-tauri && cargo build"
echo "   4. npm run tauri:dev"
echo ""

echo "📖 Documentation: docs/README.md"
echo "🔧 Configuration: config/"
echo "🧪 Tests: tests/"
echo ""

print_success "Tout est prêt ! Bon développement 🎉"
