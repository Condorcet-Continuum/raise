/**
 * Génère un ID unique court (utile pour les clés React ou IDs temporaires).
 * Pour les IDs de BDD, préférez UUID v4.
 */
export function generateId(prefix = 'id'): string {
  return `${prefix}-${Math.random().toString(36).substr(2, 9)}`;
}

/**
 * Pause l'exécution (utile pour simuler des délais API ou animations).
 */
export const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Utilitaire pour concaténer des classes CSS conditionnelles.
 * ex: cn('btn', isActive && 'active', className)
 */
export function cn(...classes: (string | undefined | null | false)[]): string {
  return classes.filter(Boolean).join(' ');
}

/**
 * Limite la fréquence d'appel d'une fonction (ex: resize, search input).
 */
export function debounce<T extends (...args: any[]) => void>(
  func: T,
  wait: number,
): (...args: Parameters<T>) => void {
  let timeout: ReturnType<typeof setTimeout>;
  return function (...args: Parameters<T>) {
    clearTimeout(timeout);
    timeout = setTimeout(() => func(...args), wait);
  };
}

/**
 * Deep clone simple (attention aux dates/fonctions, suffisant pour JSON simple).
 */
export function deepClone<T>(obj: T): T {
  if (obj === null || typeof obj !== 'object') return obj;
  return JSON.parse(JSON.stringify(obj));
}
