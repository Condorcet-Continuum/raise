/**
 * Vérifie si une chaîne est un JSON valide.
 */
export function isValidJson(str: string): boolean {
  try {
    JSON.parse(str);
    return true;
  } catch {
    return false;
  }
}

/**
 * Vérifie si une chaîne est vide ou composée uniquement d'espaces.
 */
export function isEmpty(str: string | undefined | null): boolean {
  return !str || str.trim().length === 0;
}

/**
 * Vérifie basiquement si un ID ressemble à un UUID.
 */
export function isUuid(str: string): boolean {
  const regex = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
  return regex.test(str);
}

/**
 * Vérifie si un objet possède des clés (non vide).
 */
export function hasProperties(obj: Record<string, any>): boolean {
  return obj && Object.keys(obj).length > 0;
}
