// FICHIER : src/types/rules.types.ts

export type Severity = 'Error' | 'Warning' | 'Info';

export interface ValidationIssue {
  severity: Severity;
  rule_id: string;
  element_id: string;
  message: string;
}

export type JsonValue =
  | string
  | number
  | boolean
  | null
  | { [key: string]: JsonValue }
  | JsonValue[];

// CORRECTION : Tout en minuscules (snake_case) pour matcher Serde Rust
export type Expr =
  // Valeurs de base
  | { val: JsonValue }
  | { var: string }

  // Logique
  | { and: Expr[] }
  | { or: Expr[] }
  | { not: Expr }

  // Comparaison
  | { eq: Expr[] }
  | { neq: Expr[] }
  | { gt: Expr[] }
  | { gte: Expr[] }
  | { lt: Expr[] }
  | { lte: Expr[] }

  // Math
  | { add: Expr[] }
  | { sub: Expr[] }
  | { mul: Expr[] }
  | { div: Expr[] }
  | { abs: Expr }
  | { round: { value: Expr; precision: Expr } }

  // Collections
  | { len: Expr }
  | { map: { list: Expr; alias: string; expr: Expr } }
  | { filter: { list: Expr; alias: string; condition: Expr } }
  | { contains: { list: Expr; value: Expr } }
  | { min: Expr }
  | { max: Expr }

  // String
  | { concat: Expr[] }
  | { upper: Expr }
  | { lower: Expr }
  | { trim: Expr }
  | { replace: { value: Expr; pattern: Expr; replacement: Expr } }
  | { regex_match: { value: Expr; pattern: Expr } } // Attention au snake_case ici

  // Dates
  | { now: null }
  | { date_add: { date: Expr; days: Expr } } // snake_case
  | { date_diff: { start: Expr; end: Expr } } // snake_case

  // Control
  | { if: { condition: Expr; then_branch: Expr; else_branch: Expr } } // 'if' est minuscule

  // Lookup
  | { lookup: { collection: string; id: Expr; field: string } };

export interface Rule {
  id: string;
  target: string;
  expr: Expr;
  description?: string | null;
  severity?: Severity | null;
}
