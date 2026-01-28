// FICHIER : tests/e2e/scenarios/system-modeling.spec.ts

import { test, expect } from '@playwright/test';

test.describe('System Modeling & Spatial Engine', () => {
  test.beforeEach(async ({ page }) => {
    // On charge l'application à la racine avant chaque test
    await page.goto('/');
  });

  test('should load the application correctly in 2D mode by default', async ({ page }) => {
    // Vérification que le layout principal est chargé
    await expect(page.locator('main')).toBeVisible();

    // Vérification que le moteur 3D est caché par défaut (mode 2D)
    const spatialContainer = page.locator('#spatial-canvas-container');
    await expect(spatialContainer).toBeHidden();
  });

  test('should switch to 3D View and render the Spatial Canvas', async ({ page }) => {
    // 1. Ciblage du conteneur 3D
    const spatialContainer = page.locator('#spatial-canvas-container');

    // 2. Action : On clique sur le bouton de bascule 3D
    // NOTE : Ce bouton doit avoir l'attribut data-testid="view-mode-3d"
    // Si ce bouton n'existe pas encore dans votre Header, le test échouera ici (ce qui est normal en TDD)
    const toggleButton = page.getByTestId('view-mode-3d');

    // On vérifie d'abord que le bouton existe (pour un debug clair)
    await expect(toggleButton).toBeVisible({ timeout: 5000 });
    await toggleButton.click();

    // 3. Vérification : Le conteneur 3D doit devenir visible
    await expect(spatialContainer).toBeVisible();

    // 4. Vérification : Le chargement doit se terminer
    // On attend que le texte "INITIALIZING" disparaisse
    await expect(page.getByText('INITIALIZING SPATIAL ENGINE')).toBeHidden({ timeout: 10000 });

    // 5. Vérification ultime : La balise <canvas> WebGL doit être présente dans le DOM
    const canvas = spatialContainer.locator('canvas');
    await expect(canvas).toBeAttached();
  });
});
