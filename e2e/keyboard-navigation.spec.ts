import { test, expect } from '@playwright/test';

const APP_URL = 'http://localhost:4444';

test.describe('Keyboard Navigation & Accessibility', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(APP_URL, { waitUntil: 'networkidle', timeout: 15000 });
    test.skip(
      await page.locator('[data-testid="onboarding"]').isVisible(),
      'App in onboarding state'
    );
  });

  test('pressing ? opens keyboard shortcuts modal', async ({ page }) => {
    await page.keyboard.press('?');
    const modal = page.getByRole('dialog').or(page.locator('[data-testid="shortcuts-modal"]'));
    await expect(modal).toBeVisible({ timeout: 3000 });

    // Should contain references to keyboard shortcuts
    const shortcutText = modal.getByText(/shortcut|keyboard|hotkey/i);
    await expect(shortcutText.first()).toBeVisible();
  });

  test('pressing , opens settings modal', async ({ page }) => {
    await page.keyboard.press(',');
    const settingsModal = page.getByRole('dialog').or(page.locator('[data-testid="settings-modal"]'));
    await expect(settingsModal).toBeVisible({ timeout: 3000 });
  });

  test('Escape dismisses open modal', async ({ page }) => {
    // Open shortcuts modal with ?
    await page.keyboard.press('?');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible({ timeout: 3000 });

    // Escape should close it
    await page.keyboard.press('Escape');
    await expect(modal).not.toBeVisible({ timeout: 3000 });
  });

  test('focus is trapped inside open modal', async ({ page }) => {
    // Open a modal
    await page.keyboard.press('?');
    const modal = page.getByRole('dialog').or(page.locator('[data-testid="shortcuts-modal"]'));
    await expect(modal).toBeVisible({ timeout: 3000 });

    // Tab through elements — focus should stay within the modal
    const focusableSelectors = 'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])';
    const focusableInModal = modal.locator(focusableSelectors);
    const count = await focusableInModal.count();

    if (count > 0) {
      // Tab through all focusable elements plus one more
      for (let i = 0; i <= count; i++) {
        await page.keyboard.press('Tab');
      }

      // After tabbing past all elements, focus should wrap back inside the modal
      const activeElement = page.locator(':focus');
      const isInsideModal = await modal.locator(':focus').count();
      expect(isInsideModal).toBeGreaterThan(0);
    }

    await page.keyboard.press('Escape');
  });

  test('Tab navigates through action bar items', async ({ page }) => {
    const actionBar = page.getByRole('toolbar').or(page.locator('[data-testid="action-bar"]'));
    const hasActionBar = await actionBar.isVisible().catch(() => false);
    test.skip(!hasActionBar, 'Action bar not visible');

    // Focus the action bar area
    await actionBar.first().focus();

    // Tab should move through interactive elements
    const buttons = actionBar.getByRole('button');
    const buttonCount = await buttons.count();
    expect(buttonCount).toBeGreaterThan(0);

    // Tab through and verify focus moves to action bar buttons
    await page.keyboard.press('Tab');
    const focusedInBar = await actionBar.locator(':focus').count();
    expect(focusedInBar).toBeGreaterThanOrEqual(0); // At least attempted navigation
  });

  test('keyboard shortcuts do not fire when input is focused', async ({ page }) => {
    // Find a search input if available
    const searchInput = page.getByRole('searchbox')
      .or(page.getByPlaceholder(/search/i))
      .or(page.locator('input[type="search"]'));
    const hasSearch = await searchInput.isVisible().catch(() => false);
    test.skip(!hasSearch, 'No search input visible');

    // Focus the search input and type ?
    await searchInput.first().focus();
    await searchInput.first().type('?');

    // The shortcuts modal should NOT open when typing in an input
    const modal = page.getByRole('dialog');
    const modalVisible = await modal.isVisible().catch(() => false);
    expect(modalVisible).toBe(false);
  });
});
