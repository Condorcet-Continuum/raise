import { useEffect } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

/**
 * S'abonne à un événement Tauri et nettoie l'écouteur au démontage du composant.
 * @param eventName Le nom de l'événement (ex: "backend-log")
 * @param handler La fonction callback
 */
export function useTauriEvent<T = unknown>(eventName: string, handler: (payload: T) => void) {
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    // On initialise l'écoute
    const setupListener = async () => {
      try {
        unlisten = await listen<T>(eventName, (event) => {
          handler(event.payload);
        });
      } catch (err) {
        console.error(`[useTauriEvent] Échec écoute "${eventName}":`, err);
      }
    };

    setupListener();

    // Nettoyage automatique
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [eventName, handler]);
}
