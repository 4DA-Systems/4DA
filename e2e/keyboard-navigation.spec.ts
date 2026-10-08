import { test, expect } from '@playwright/test';

import { appBar, gotoMainShell } from './support/app';

test.describe('Keyboard Navigation & Accessibility', () => {
  test.beforeEach(async ({ page }) => {
    await gotoMainShell(page);
  });

  test('pressing ? opens keyboard shortcuts modal', async ({ page }) => {
    await page.keyboard.press('?');
    const modal = page.getByRole('dialog').or(page.locator('[data-testid="shortcuts-modal"]'));
    await expect(modal).toBeVisible();

    // Should contain references to keyboard shortcuts
    const shortcutText = modal.getByText(/shortcut|keyboard|hotkey/i);
    await expect(shortcutText.first()).toBeVisible();
  });

  test('pressing , opens settings modal', async ({ page }) => {
    await page.keyboard.press(',');
    const settingsModal = page.getByRole('dialog').or(page.locator('[data-testid="settings-modal"]'));
    await expect(settingsModal).toBeVisible();
  });

  test('Escape dismisses open modal', async ({ page }) => {
    // Open shortcuts modal with ?
    await page.keyboard.press('?');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible();

    // Escape should close it
    await page.keyboard.press('Escape');
    await expect(modal).not.toBeVisible();
  });

  test('focus is trapped inside open modal', async ({ page }) => {
    // Open a modal
    await page.keyboard.press('?');
    const modal = page.getByRole('dialog').or(page.locator('[data-testid="shortcuts-modal"]'));
    await expect(modal).toBeVisible();

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

  test('Tab navigates through app bar controls in order', async ({ page }) => {
    // The analysis controls live in the app bar (banner), not a toolbar.
    const bar = appBar(page);
    await bar.getByRole('combobox', { name: 'Search 4DA' }).focus();

    await page.keyboard.press('Tab');
    await expect(bar.getByRole('button', { name: 'Run analysis' })).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(bar.getByRole('button', { name: /switch to (light|dark) theme/i })).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(bar.getByRole('button', { name: 'Settings', exact: true })).toBeFocused();
  });

  test('keyboard shortcuts do not fire when input is focused', async ({ page }) => {
    // Find a search input if available
    const searchInput = appBar(page).getByRole('combobox', { name: 'Search 4DA' });
    await searchInput.focus();
    await searchInput.pressSequentially('?,');

    // Neither the shortcuts (?) nor the settings (,) dialog opens; the
    // characters go into the input.
    await expect(searchInput).toHaveValue('?,');
    await expect(page.getByRole('dialog')).toHaveCount(0);
  });
});
