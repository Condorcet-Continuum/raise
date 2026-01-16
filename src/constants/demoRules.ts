// FICHIER : src/constants/demoRules.ts

import { Rule } from '../types/rules.types';

export const DEMO_RULES: Record<string, Record<string, Rule>> = {
  invoices: {
    due_at: {
      id: 'calc_due_date',
      target: 'due_at',
      expr: {
        date_add: {
          // minuscules
          date: { var: 'created_at' },
          days: { var: 'days' },
        },
      },
    },
    ref: {
      id: 'gen_ref',
      target: 'ref',
      expr: {
        concat: [
          // minuscules
          { val: 'INV-' },
          { upper: { var: 'user_id' } },
          { val: '-' },
          { var: 'days' },
        ],
      },
    },
  },
  components: {
    compliance: {
      id: 'check_naming',
      target: 'compliance',
      expr: {
        if: {
          // minuscules
          condition: { regex_match: { value: { var: 'name' }, pattern: { val: '^[A-Z]' } } }, // regex_match
          then_branch: { val: '✅ VALIDE (PascalCase)' },
          else_branch: { val: '❌ NON_CONFORME (Doit commencer par Majuscule)' },
        },
      },
    },
    full_path: {
      id: 'gen_path',
      target: 'full_path',
      expr: {
        concat: [{ var: 'parent_pkg' }, { val: '.' }, { var: 'name' }],
      },
    },
  },
};
