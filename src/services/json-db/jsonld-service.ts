// src/services/json-db/jsonld-service.ts

/**
 * Service de gestion JSON-LD pour données liées (Client Side).
 * Utile pour pré-traiter les données avant de les envoyer au backend ou pour l'affichage.
 */

export interface JsonLdContext {
  '@context': ContextDefinition;
}

export type ContextDefinition = string | Record<string, ContextValue> | ContextDefinition[];

export type ContextValue =
  | string
  | {
      '@id': string;
      '@type'?: string;
      '@container'?: string;
    };

export class JsonLdService {
  private readonly contexts = new Map<string, JsonLdContext>();

  /**
   * Enregistre un contexte nommé en mémoire client.
   * ex: registerContext("arcadia", { "@context": { "oa": "http://...", ... } })
   */
  registerContext(name: string, context: JsonLdContext): void {
    this.contexts.set(name, context);
  }

  getContext(name: string): JsonLdContext | undefined {
    return this.contexts.get(name);
  }

  /**
   * "Expand" léger : ajoute @context au document.
   */
  expandDocument<T = any>(document: T, contextName: string): T & { '@context': any } {
    const context = this.getContext(contextName);
    if (!context) {
      console.warn(`JSON-LD context not found client-side: ${contextName}`);
      return { '@context': {}, ...document };
    }
    return {
      '@context': context['@context'],
      ...document,
    };
  }

  /**
   * "Compact" léger : retire la clé @context pour un affichage propre.
   */
  compactDocument<T = any>(document: any): T {
    if (!document) return document;
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    const { '@context': _, ...rest } = document;
    return rest as T;
  }
}

export const jsonLdService = new JsonLdService();
