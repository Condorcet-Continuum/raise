import { useRulesEngine } from '@/hooks/useRulesEngine';
import DebugConsole from './DebugConsole';

interface InvoiceData {
  user_id?: string;
  days?: number;
  created_at?: string;
  due_at?: string;
  ref?: string;
  amount?: number;
  [key: string]: unknown;
}

export default function InvoiceDemo() {
  const { doc, handleChange, isCalculating, logs } = useRulesEngine<InvoiceData>({
    space: 'demo',
    db: 'finance',
    collection: 'invoices',
    initialDoc: {
      user_id: 'USR_123',
      days: 30,
      created_at: new Date().toISOString(),
    },
  });

  const data = doc;
  const inputClass =
    'mt-1 block w-full rounded-lg border-0 py-2 px-3 text-gray-900 shadow-sm ring-1 ring-inset ring-gray-300 placeholder:text-gray-400 focus:ring-2 focus:ring-inset focus:ring-indigo-600 sm:text-sm sm:leading-6 transition-shadow';
  const labelClass = 'block text-xs font-semibold leading-6 text-gray-500 uppercase tracking-wide';

  return (
    <div className="space-y-6">
      <div className="bg-white rounded-2xl shadow-sm border border-slate-200 overflow-hidden">
        <div className="border-b border-slate-100 bg-slate-50/50 px-6 py-4 flex justify-between items-center">
          <div>
            <h3 className="text-base font-semibold leading-6 text-slate-900">🧾 Facturation</h3>
            <p className="text-sm text-slate-500">Calculs de dates et concaténation.</p>
          </div>
          {/* UTILISATION DE isCalculating ICI */}
          {isCalculating && (
            <span className="inline-flex items-center rounded-md bg-yellow-50 px-2 py-1 text-xs font-medium text-yellow-800 ring-1 ring-inset ring-yellow-600/20 animate-pulse">
              Calcul en cours...
            </span>
          )}
        </div>

        <div className="p-6 grid grid-cols-1 md:grid-cols-2 gap-8">
          <div className="space-y-5">
            <div>
              <label className={labelClass}>ID Utilisateur</label>
              <input
                type="text"
                className={inputClass}
                value={data.user_id || ''}
                onChange={(e) => handleChange('user_id', e.target.value)}
              />
            </div>
            <div>
              <label className={labelClass}>Délai (jours)</label>
              <input
                type="number"
                className={inputClass}
                value={data.days || 0}
                onChange={(e) => handleChange('days', parseInt(e.target.value) || 0)}
              />
            </div>
            <div>
              <label className={labelClass}>Date Création</label>
              <input
                type="date"
                className={inputClass}
                value={typeof data.created_at === 'string' ? data.created_at.split('T')[0] : ''}
                onChange={(e) => handleChange('created_at', new Date(e.target.value).toISOString())}
              />
            </div>
          </div>
          <div className="bg-slate-50 rounded-xl p-5 border border-slate-200 flex flex-col justify-center">
            <div className="space-y-4">
              <div className="bg-white p-4 rounded-lg border border-slate-100 shadow-sm">
                <span className="text-xs text-slate-400 block mb-1">Date d'échéance</span>
                <span className="text-xl font-mono font-bold text-indigo-600">
                  {data.due_at ? new Date(data.due_at).toLocaleDateString() : '...'}
                </span>
              </div>
              <div className="bg-white p-4 rounded-lg border border-slate-100 shadow-sm">
                <span className="text-xs text-slate-400 block mb-1">Référence</span>
                <code className="text-sm font-mono bg-slate-100 px-2 py-1 rounded text-slate-700">
                  {data.ref || '...'}
                </code>
              </div>
            </div>
          </div>
        </div>
      </div>

      <DebugConsole logs={logs} />
    </div>
  );
}
