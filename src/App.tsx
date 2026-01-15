// FICHIER : src/App.tsx

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './styles/globals.css';

// --- UTILS & STORES ---
import { MOCK_PROJECT } from '@/utils/mock-data';
import { useModelStore } from '@/store/model-store';

// --- LAYOUT ---
import { MainLayout } from '@/components/layout/MainLayout';

// --- VUES ---
import CapellaViewer from '@/components/model-viewer/CapellaViewer';
import GeneticsDashboard from '@/components/genetics/GeneticsDashboard';
import CodeGenerator from '@/components/codegen/CodeGenerator';
import DiagramCanvas from '@/components/diagram-editor/DiagramCanvas';
import WorkflowCanvas from '@/components/workflow-designer/WorkflowCanvas';
//import CognitiveAnalysis from '@/components/cognitive/CognitiveAnalysis';
import UseCaseManager from '@/components/cognitive/UseCaseManager';
import AssuranceDashboard from '@/components/assurance/AssuranceDashboard';
import MBAIEView from '@/components/ai-chat/MBAIEView';
import SettingsPage from '@/components/settings/SettingsPage';
import { JsonDbTester } from '@/components/JsonDbTester';
import CognitiveTester from '@/components/CognitiveTester';
import RulesEngineDashboard from '@/components/rules_engine/RulesEngineDashboard';
import BlockchainView from '@/components/blockchain/BlockchainView';
import DashboardView from '@/components/dashboard/DashboardView';

// --- GOUVERNANCE ---
// On importe le composant depuis features
import GovernanceConsole from '@/features/governance/GovernanceConsole';

import { DeepLearningPanel } from '@/components/DeepLearning/DeepLearningPanel';

// --- TYPAGE STRICT ---
interface SystemInfo {
  app_version: string;
  env_mode: string;
  database_path: string;
  api_status: string;
}

export default function App() {
  const [currentPage, setCurrentPage] = useState('dashboard');

  // Correction : Remplacement de any par SystemInfo | null
  const [sysInfo, setSysInfo] = useState<SystemInfo | null>(null);

  const { setProject } = useModelStore();

  useEffect(() => {
    console.log('🚀 Démarrage de RAISE...');

    // On type le retour de l'invoke
    invoke<SystemInfo>('get_app_info')
      .then((response) => setSysInfo(response))
      .catch((error: unknown) => console.error('❌ Erreur Backend Rust :', error));

    const timer = setTimeout(() => {
      setProject(MOCK_PROJECT);
    }, 500);
    return () => clearTimeout(timer);
  }, [setProject]);

  // ... (Le reste du code renderContent et getTitle reste inchangé)
  // Je remets juste le bloc de rendu pour la complétude
  const renderContent = () => {
    switch (currentPage) {
      case 'governance':
        return <GovernanceConsole />;
      case 'model':
        return <CapellaViewer />;
      case 'genetics':
        return <GeneticsDashboard />;
      case 'codegen':
        return <CodeGenerator />;
      case 'diagram':
        return <DiagramCanvas />;
      case 'workflow':
        return <WorkflowCanvas />;
      case 'cognitive':
        return <UseCaseManager />;
      case 'assurance':
        return <AssuranceDashboard />;
      case 'ai':
        return <MBAIEView />;
      case 'settings':
        return <SettingsPage />;
      case 'admin-db':
        return <JsonDbTester />;
      case 'cognitive-tester':
        return <CognitiveTester />;
      case 'rules-engine':
        return <RulesEngineDashboard />;
      case 'blockchain':
        return <BlockchainView />;

      case 'deep-learning':
        return <DeepLearningPanel />;

      case 'dashboard':
      default:
        return <DashboardView sysInfo={sysInfo} onNavigate={setCurrentPage} />;
    }
  };

  const getTitle = () => {
    switch (currentPage) {
      case 'model':
        return 'Modélisation Arcadia';
      case 'genetics':
        return 'Optimisation Génétique';
      case 'codegen':
        return 'Génération de Code';
      case 'ai':
        return 'MBAIE';
      case 'diagram':
        return 'Éditeur de Diagrammes';
      case 'workflow':
        return 'Workflow Designer';
      case 'blockchain':
        return 'Blockchain Ledger';
      case 'cognitive':
        return 'Blocs Cognitifs';
      case 'assurance':
        return 'Product Assurance & XAI';
      case 'settings':
        return 'Paramètres Système';
      case 'admin-db':
        return 'Gestion de la DB';
      case 'cognitive-tester':
        return 'Diagnostic Cognitif (WASM)';
      case 'rules-engine':
        return 'Moteur de Règles (GenRules)';
      default:
        return 'RAISE';
    }
  };

  return (
    <MainLayout currentPage={currentPage} onNavigate={setCurrentPage} pageTitle={getTitle()}>
      {renderContent()}
    </MainLayout>
  );
}
