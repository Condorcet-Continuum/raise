import { useState, useEffect, useCallback } from 'react';
import { dryRunRule } from '../services/rulesService';
import { DEMO_RULES } from '../constants/demoRules';
import { JsonValue } from '../types/rules.types'; // Suppression de 'Rule'

interface UseRulesEngineProps<T> {
  space: string;
  db: string;
  collection: string;
  initialDoc: T;
}

export interface RuleExecutionLog {
  id: string;
  timestamp: string;
  ruleId: string;
  targetField: string;
  status: 'success' | 'error';
  result?: JsonValue;
  details?: string;
}

export function useRulesEngine<T extends Record<string, unknown>>({
  collection,
  initialDoc,
}: UseRulesEngineProps<T>) {
  const [doc, setDoc] = useState<T>(initialDoc);
  const [isCalculating, setIsCalculating] = useState(false);
  const [logs, setLogs] = useState<RuleExecutionLog[]>([]);

  const addLog = (log: Omit<RuleExecutionLog, 'id' | 'timestamp'>) => {
    const newLog: RuleExecutionLog = {
      id: Math.random().toString(36).substr(2, 9),
      timestamp: new Date().toLocaleTimeString(),
      ...log,
    };
    setLogs((prev) => [newLog, ...prev].slice(0, 20));
  };

  const runRules = useCallback(
    async (currentDoc: T) => {
      const rulesMap = DEMO_RULES[collection];
      if (!rulesMap) return;

      setIsCalculating(true);

      try {
        const updates: Partial<T> = {};
        const promises = Object.values(rulesMap).map(async (rule) => {
          try {
            const context = currentDoc as Record<string, unknown>;
            const start = performance.now();
            const result = await dryRunRule(rule, context);
            const duration = (performance.now() - start).toFixed(2);

            addLog({
              ruleId: rule.id,
              targetField: rule.target,
              status: 'success',
              result: result,
              details: `Calculé en ${duration}ms via Rust`,
            });

            return { field: rule.target, value: result };
          } catch (err) {
            addLog({
              ruleId: rule.id,
              targetField: rule.target,
              status: 'error',
              details: String(err),
            });
            return null;
          }
        });

        const results = await Promise.all(promises);

        results.forEach((res) => {
          if (res) {
            updates[res.field as keyof T] = res.value as T[keyof T];
          }
        });

        setDoc((prev) => ({ ...prev, ...updates }));
      } catch (err) {
        console.error(err);
      } finally {
        setIsCalculating(false);
      }
    },
    [collection],
  );

  const handleChange = (field: keyof T, value: JsonValue) => {
    setDoc((prev) => {
      const newDoc = { ...prev, [field]: value };
      runRules(newDoc);
      return newDoc;
    });
  };

  useEffect(() => {
    runRules(initialDoc);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return {
    doc,
    handleChange,
    isCalculating,
    logs,
  };
}
