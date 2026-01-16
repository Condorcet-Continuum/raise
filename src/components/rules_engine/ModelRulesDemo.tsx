import { useRulesEngine } from '@/hooks/useRulesEngine';
import DebugConsole from './DebugConsole';

interface ModelRuleData {
  name?: string;
  parent_pkg?: string;
  description?: string;
  compliance?: string;
  full_path?: string;
  [key: string]: unknown;
}

export default function ModelRulesDemo() {
  const { doc, handleChange, isCalculating, logs } = useRulesEngine<ModelRuleData>({
    space: 'demo',
    db: 'architecture',
    collection: 'components',
    initialDoc: {
      name: 'UserSystem',
      parent_pkg: 'com.company.core',
      description: '',
    },
  });

  const data = doc;
  const isCompliant = typeof data.compliance === 'string' && data.compliance.includes('VALIDE');
  const inputClass =
    'mt-1 block w-full rounded-lg border-0 py-2 px-3 text-gray-900 shadow-sm ring-1 ring-inset ring-gray-300 placeholder:text-gray-400 focus:ring-2 focus:ring-inset focus:ring-indigo-600 sm:text-sm sm:leading-6 transition-shadow';
  const labelClass = 'block text-xs font-semibold leading-6 text-gray-500 uppercase tracking-wide';

  return (
    <div className="space-y-6">
      <div className="bg-white rounded-2xl shadow-sm border border-slate-200 overflow-hidden">
        <div className="border-b border-slate-100 bg-slate-50/50 px-6 py-4 flex justify-between items-center">
          <div>
            <h3 className="text-base font-semibold leading-6 text-slate-900">🏗️ Architecture</h3>
            <p className="text-sm text-slate-500">Validation temps-réel (Regex & AST).</p>
          </div>
          {/* UTILISATION DE isCalculating */}
          {isCalculating && (
            <span className="inline-flex items-center rounded-md bg-indigo-50 px-2 py-1 text-xs font-medium text-indigo-700 ring-1 ring-inset ring-indigo-700/10 animate-pulse">
              Verification...
            </span>
          )}
        </div>
        <div className="p-6 grid grid-cols-1 md:grid-cols-2 gap-8">
          <div className="space-y-5">
            <div>
              <label className={labelClass}>Package Parent</label>
              <input
                type="text"
                className={inputClass}
                value={data.parent_pkg || ''}
                onChange={(e) => handleChange('parent_pkg', e.target.value)}
              />
            </div>
            <div>
              <label className={labelClass}>Nom du Composant</label>
              <input
                type="text"
                className={inputClass}
                value={data.name || ''}
                onChange={(e) => handleChange('name', e.target.value)}
              />
              <p className="mt-1 text-xs text-slate-400">Règle : PascalCase (Ex: MySystem)</p>
            </div>
          </div>
          <div className="flex flex-col gap-4">
            <div
              className={`flex-1 rounded-xl border p-6 flex flex-col items-center justify-center text-center transition-colors ${
                isCompliant ? 'bg-emerald-50 border-emerald-100' : 'bg-rose-50 border-rose-100'
              }`}
            >
              {/* Animation si calcul en cours */}
              <span className={`text-4xl mb-2 ${isCalculating ? 'animate-bounce opacity-50' : ''}`}>
                {isCompliant ? '🛡️' : '⚠️'}
              </span>
              <span
                className={`font-bold text-sm ${
                  isCompliant ? 'text-emerald-700' : 'text-rose-700'
                }`}
              >
                {isCalculating ? 'Audit...' : data.compliance || 'En attente...'}
              </span>
            </div>
            <div className="bg-slate-900 rounded-lg p-4 shadow-inner">
              <label className="text-[10px] text-slate-400 uppercase tracking-widest block mb-2">
                Chemin complet
              </label>
              <code className="font-mono text-sm text-green-400">{data.full_path || '...'}</code>
            </div>
          </div>
        </div>
      </div>

      <DebugConsole logs={logs} />
    </div>
  );
}
