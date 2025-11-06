/**
 * Service de gestion JSON-LD pour données liées
 */

export interface JsonLdContext {
  '@context': ContextDefinition;
}

export type ContextDefinition = string | Record<string, ContextValue> | ContextDefinition[];

export type ContextValue = string | {
  '@id': string;
  '@type'?: string;
  '@container'?: string;
};

export class JsonLdService {
  private contexts: Map<string, JsonLdContext> = new Map();

  registerContext(name: string, context: JsonLdContext): void {
    this.contexts.set(name, context);
  }

  getContext(name: string): JsonLdContext | undefined {
    return this.contexts.get(name);
  }

  expandDocument(document: any, contextName: string): any {
    const context = this.getContext(contextName);
    if (!context) {
      return document;
    }

    // TODO: Implémenter l'expansion JSON-LD
    return {
      '@context': context['@context'],
      ...document
    };
  }

  compactDocument(document: any, contextName: string): any {
    // TODO: Implémenter la compaction JSON-LD
    const { '@context': _, ...data } = document;
    return data;
  }
}

export const jsonLdService = new JsonLdService();
