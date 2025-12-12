/**
 * Convertit un tableau d'objets en Map (Record) indexé par une clé (souvent 'id').
 * Utile pour l'optimisation des stores Zustand.
 */
export function arrayToRecord<T>(array: T[], key: keyof T): Record<string, T> {
  return array.reduce((acc, item) => {
    const id = item[key] as unknown as string;
    acc[id] = item;
    return acc;
  }, {} as Record<string, T>);
}

/**
 * Convertit une couleur Hexadécimale en RGBA.
 * Utile pour appliquer de l'opacité sur une couleur de thème.
 * ex: hexToRgba('#ffffff', 0.5) -> 'rgba(255, 255, 255, 0.5)'
 */
export function hexToRgba(hex: string, alpha: number): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);

  if (isNaN(r) || isNaN(g) || isNaN(b)) return `rgba(0, 0, 0, ${alpha})`;

  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/**
 * Convertit une chaîne CamelCase en Snake_case (pour Rust).
 */
export function camelToSnakeCase(str: string): string {
  return str.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}
