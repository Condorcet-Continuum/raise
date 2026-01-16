import { RuleExecutionLog } from '@/hooks/useRulesEngine';

interface DebugConsoleProps {
  logs: RuleExecutionLog[];
  isOpen?: boolean;
}

export default function DebugConsole({ logs }: DebugConsoleProps) {
  return (
    <div className="mt-6 bg-slate-900 rounded-xl overflow-hidden border border-slate-700 shadow-2xl font-mono text-xs">
      {/* Header Console */}
      <div className="bg-slate-950 px-4 py-2 border-b border-slate-800 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-emerald-500">➜</span>
          <span className="text-slate-300 font-bold">Rust Engine Activity</span>
        </div>
        <div className="text-slate-500">Live Stream</div>
      </div>

      {/* Logs Area */}
      <div className="p-4 h-48 overflow-y-auto space-y-3 custom-scrollbar">
        {logs.length === 0 ? (
          <div className="text-slate-600 italic text-center py-10">
            En attente d'événements... modifiez un champ pour déclencher le moteur.
          </div>
        ) : (
          logs.map((log) => (
            <div
              key={log.id}
              className="border-l-2 border-slate-700 pl-3 py-1 animate-in slide-in-from-left-2 duration-300"
            >
              <div className="flex items-center gap-3 mb-1">
                <span className="text-slate-500">[{log.timestamp}]</span>
                <span
                  className={`font-bold ${
                    log.status === 'success' ? 'text-emerald-400' : 'text-rose-400'
                  }`}
                >
                  {log.status === 'success' ? 'EXEC_OK' : 'EXEC_ERR'}
                </span>
                <span className="text-indigo-300">Rule: {log.ruleId}</span>
              </div>

              <div className="flex items-start gap-2 text-slate-300">
                <span className="text-slate-500">Target:</span>
                <span className="text-yellow-100">{log.targetField}</span>

                <span className="text-slate-600 mx-1">➜</span>

                <span className="text-slate-500">Result:</span>
                <span className="text-cyan-300 break-all">
                  {typeof log.result === 'object' ? JSON.stringify(log.result) : String(log.result)}
                </span>
              </div>

              {log.details && <div className="text-slate-500 mt-0.5 ml-1">└ {log.details}</div>}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
