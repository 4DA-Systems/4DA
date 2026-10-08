import { test, expect } from '@playwright/test';

import { gotoMainShell } from './support/app';

// Dialog waits use the suite's expect timeout (playwright.config.ts). The old
// 3 s overrides measured machine load, not behaviour: SettingsModal is a lazy
// chunk (AppModals.tsx), every test's fresh browser context re-fetches its
// unbundled dev-server module graph, and under a parallel cargo build that
// missed 3 s on a warm server (open via ',' and via the header button alike).
test.describe('Settings Modal Roundtrip', () => {
  test.beforeEach(async ({ page }) => {
    await gotoMainShell(page);
  });

  test('settings opens via header button click', async ({ page }) => {
    const settingsButton = page.locator('button[aria-label*="settings" i]')
      .or(page.getByRole('button', { name: /settings|preferences|gear/i }))
      .or(page.locator('[data-testid="settings-button"]'));
    await expect(settingsButton.first()).toBeVisible({ timeout: 10000 });

    await settingsButton.first().click();

    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible();
  });

  test('all settings tabs are present and visible', async ({ page }) => {
    // Open settings
    await page.keyboard.press(',');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible();

    // Look for tab elements within the modal
    const tabs = modal.getByRole('tab');
    const tabCount = await tabs.count();
    expect(tabCount).toBeGreaterThanOrEqual(2); // At least 2 tabs expected
  });

  test('settings tabs are navigable via click', async ({ page }) => {
    await page.keyboard.press(',');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible();

    const tabs = modal.getByRole('tab');
    const tabCount = await tabs.count();
    test.skip(tabCount < 2, 'Not enough tabs to test navigation');

    // Click the second tab
    await tabs.nth(1).click();
    await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');

    // Click back to first tab
    await tabs.nth(0).click();
    await expect(tabs.nth(0)).toHaveAttribute('aria-selected', 'true');
  });

  test('settings closes via close button', async ({ page }) => {
    await page.keyboard.press(',');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible();

    // Find close button — could be X button or explicit close
    const closeButton = modal.getByRole('button', { name: /close|dismiss/i })
      .or(modal.locator('button[aria-label*="close" i]'))
      .or(modal.locator('[data-testid="close-button"]'));
    await closeButton.first().click();

    await expect(modal).not.toBeVisible();
  });

  test('settings closes via Escape key', async ({ page }) => {
    await page.keyboard.press(',');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible();

    await page.keyboard.press('Escape');
    await expect(modal).not.toBeVisible();
  });

  test('toggle switches are interactive', async ({ page }) => {
    await page.keyboard.press(',');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible();

    // General has no toggle without a backend (monitoring status never
    // loads); the LLM re-ranking checkbox on Intelligence is pure form state.
    await modal.getByRole('tab', { name: 'Intelligence', exact: true }).click();
    const toggle = modal.getByRole('checkbox', { name: 'Enable LLM re-ranking' });
    await expect(toggle).toBeVisible();
    const initialState = await toggle.isChecked();

    await toggle.click();
    await expect(toggle).toBeChecked({ checked: !initialState });

    // Toggle back to restore original state
    await toggle.click();
    await expect(toggle).toBeChecked({ checked: initialState });
  });

  test('About tab shows app information', async ({ page }) => {
    await page.keyboard.press(',');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible();

    // Navigate to About tab
    const aboutTab = modal.getByRole('tab', { name: 'About', exact: true });
    await aboutTab.click();
    await expect(aboutTab).toHaveAttribute('aria-selected', 'true');

    // About shows the app identity
    const panel = modal.getByRole('tabpanel', { name: 'About' });
    await expect(panel.getByRole('heading', { name: '4DA', level: 3 })).toBeVisible();
    await expect(panel.getByText('4 Dimensional Autonomy')).toBeVisible();
  });

  test('settings can be reopened after closing', async ({ page }) => {
    // First open
    await page.keyboard.press(',');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible();

    // Close
    await page.keyboard.press('Escape');
    await expect(modal).not.toBeVisible();

    // Reopen
    await page.keyboard.press(',');
    await expect(modal).toBeVisible();

    // Should still be functional — tabs should render
    const tabs = modal.getByRole('tab');
    const tabCount = await tabs.count();
    expect(tabCount).toBeGreaterThanOrEqual(1);

    // Clean up
    await page.keyboard.press('Escape');
  });
});
