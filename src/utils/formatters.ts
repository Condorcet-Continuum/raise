/**
 * Formate une date ISO en format local court.
 * ex: "2023-10-01T12:00:00Z" -> "01/10/2023 14:00"
 */
export function formatDate(isoString: string | number | Date): string {
  if (!isoString) return '-';
  const date = new Date(isoString);
  return new Intl.DateTimeFormat('fr-FR', {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date);
}

/**
 * Transforme le PascalCase en mots séparés.
 * ex: "LogicalComponent" -> "Logical Component"
 * ex: "OperationalActor" -> "Operational Actor"
 */
export function formatArcadiaType(type: string): string {
  if (!type) return '';
  // Retire l'URI si présente (ex: http://...#Type)
  const cleanType = type.split('#').pop() || type;
  // Ajoute un espace avant les majuscules
  return cleanType.replace(/([A-Z])/g, ' $1').trim();
}

/**
 * Formate un nombre d'octets en taille lisible (KB, MB).
 */
export function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

/**
 * Tronque un texte s'il dépasse une certaine longueur.
 */
export function truncate(str: string, length: number): string {
  if (!str) return '';
  if (str.length <= length) return str;
  return str.slice(0, length) + '...';
}
