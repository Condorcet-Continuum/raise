import { useState, ReactNode } from 'react';
import InvoiceDemo from './InvoiceDemo';
import ModelRulesDemo from './ModelRulesDemo';

type ViewState = 'home' | 'model' | 'invoice';

// Définition propre des props pour éviter le 'any'
interface LaunchCardProps {
  title: string;
  desc: string;
  onClick: () => void;
  icon: ReactNode;
  color: 'indigo' | 'emerald';
}

export default function RulesEngineDashboard() {
  const [currentView, setCurrentView] = useState<ViewState>('home');

  // --- VUE DÉTAIL (Simulateur) ---
  if (currentView !== 'home') {
    return (
      <div className="h-full flex flex-col bg-slate-50 font-sans">
        {/* Barre de navigation simple */}
        <div className="bg-white border-b border-slate-200 px-6 py-3 flex items-center justify-between sticky top-0 z-20">
          <button
            onClick={() => setCurrentView('home')}
            className="flex items-center gap-2 text-slate-500 hover:text-indigo-600 transition-colors text-sm font-medium"
          >
            <span className="bg-slate-100 hover:bg-indigo-50 p-1.5 rounded-md transition-colors">
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M10 19l-7-7m0 0l7-7m-7 7h18"
                />
              </svg>
            </span>
            Retour
          </button>

          <div className="flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-green-500"></span>
            <span className="text-sm font-semibold text-slate-700">
              {currentView === 'model' ? 'Architecture' : 'Facturation'}
            </span>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-6">
          <div className="max-w-3xl mx-auto">
            {currentView === 'model' ? <ModelRulesDemo /> : <InvoiceDemo />}
          </div>
        </div>
      </div>
    );
  }

  // --- VUE ACCUEIL (Grille) ---
  return (
    <div className="h-full w-full bg-slate-50 p-10 overflow-y-auto flex flex-col items-center">
      <div className="text-center max-w-xl mb-10">
        <h1 className="text-3xl font-bold text-slate-900 mb-2">Moteur de Règles</h1>
        <p className="text-slate-500">Sélectionnez un module pour tester l'AST Rust.</p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6 w-full max-w-4xl">
        <LaunchCard
          onClick={() => setCurrentView('model')}
          title="Architecture & Ingénierie"
          desc="Validation de conformité (Regex, Naming)."
          icon={
            <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"
              />
            </svg>
          }
          color="indigo"
        />
        <LaunchCard
          onClick={() => setCurrentView('invoice')}
          title="Finance & Facturation"
          desc="Calculs mathématiques et dates."
          icon={
            <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M9 7h6m0 10v-3m-3 3h.01M9 17h.01M9 14h.01M12 14h.01M15 11h.01M12 11h.01M9 11h.01M7 21h10a2 2 0 002-2V5a2 2 0 00-2-2H7a2 2 0 00-2 2v14a2 2 0 002 2z"
              />
            </svg>
          }
          color="emerald"
        />
      </div>
    </div>
  );
}

function LaunchCard({ title, desc, onClick, icon, color }: LaunchCardProps) {
  const colors = {
    indigo: 'bg-indigo-50 text-indigo-600 group-hover:bg-indigo-600 group-hover:text-white',
    emerald: 'bg-emerald-50 text-emerald-600 group-hover:bg-emerald-600 group-hover:text-white',
  };

  return (
    <button
      onClick={onClick}
      className="group bg-white p-6 rounded-2xl shadow-sm border border-slate-200 hover:border-slate-300 hover:shadow-md transition-all text-left flex items-start gap-4"
    >
      <div className={`p-3 rounded-xl transition-colors ${colors[color]}`}>{icon}</div>
      <div>
        <h3 className="font-bold text-slate-800 group-hover:text-indigo-600 transition-colors">
          {title}
        </h3>
        <p className="text-sm text-slate-500 mt-1">{desc}</p>
      </div>
    </button>
  );
}
