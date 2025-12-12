/**
 * Service pont pour charger et interagir avec des modules WASM côté client (Navigateur).
 * Note: La plupart des logiques lourdes sont actuellement gérées côté Rust via Tauri.
 * Ce fichier servira si nous voulons exécuter des analyses légères sans round-trip IPC.
 */

export class WasmBridge {
  private instance: WebAssembly.Instance | null = null;

  async loadModule(path: string) {
    try {
      const response = await fetch(path);
      const bytes = await response.arrayBuffer();
      const { instance } = await WebAssembly.instantiate(bytes, {
        env: {
          // Import functions if needed
          console_log: (arg: any) => console.log(arg),
        },
      });
      this.instance = instance;
      console.log(`[WasmBridge] Module ${path} loaded.`);
    } catch (e) {
      console.error(`[WasmBridge] Failed to load ${path}`, e);
    }
  }

  // Exemple générique d'appel
  run(functionName: string, ...args: number[]) {
    if (!this.instance) {
      console.warn('[WasmBridge] No module loaded.');
      return;
    }
    const func = this.instance.exports[functionName] as CallableFunction;
    if (func) {
      return func(...args);
    }
  }
}

export const wasmBridge = new WasmBridge();
