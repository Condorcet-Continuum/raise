// src/services/json-db/jsonld-service.ts

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

  registerContext(name: string, context: JsonLdContext): void {
    this.contexts.set(name, context);
  }

  getContext(name: string): JsonLdContext | undefined {
    return this.contexts.get(name);
  }

  expandDocument<T = any>(document: T, contextName: string): T & { '@context': any } {
    const context = this.getContext(contextName);
    if (!context) {
      console.warn(`[JsonLdService] Context not found: ${contextName}`);
      return { '@context': {}, ...document };
    }
    return {
      '@context': context['@context'],
      ...document,
    };
  }

  compactDocument<T = any>(document: any): T {
    if (!document) return document;
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    const { '@context': _, ...rest } = document;
    return rest as T;
  }
}

export const jsonLdService = new JsonLdService();
