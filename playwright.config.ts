// FICHIER : playwright.config.ts

import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  // 1. IMPORTANT : On cible uniquement le dossier E2E
  testDir: './tests/e2e',

  // 2. Exécution en parallèle
  fullyParallel: true,

  // 3. Ne pas arrêter les tests si un seul échoue
  forbidOnly: !!process.env.CI,

  // 4. Nombre de tentatives (retries)
  retries: process.env.CI ? 2 : 0,

  // 5. Configuration des rapports (html pour voir les résultats)
  reporter: 'html',

  // 6. Configuration commune
  use: {
    // URL de base de votre appli (Tauri en dev tourne souvent sur localhost:1420)
    baseURL: 'http://localhost:1420',

    // Collecte des traces en cas d'erreur (très utile pour débugger)
    trace: 'on-first-retry',
  },

  // 7. Configuration des navigateurs
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    // Vous pouvez décommenter Firefox si besoin, mais Chrome suffit pour le dév
    /*
    {
      name: 'firefox',
      use: { ...devices['Desktop Firefox'] },
    },
    */
  ],

  // 8. (Optionnel) Lancer le serveur de dév automatiquement avant les tests
  /*
  webServer: {
    command: 'npm run tauri dev',
    url: 'http://localhost:1420',
    reuseExistingServer: !process.env.CI,
  },
  */
});
