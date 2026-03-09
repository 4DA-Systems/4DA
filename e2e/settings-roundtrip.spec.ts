import { test, expect } from '@playwright/test';

const APP_URL = 'http://localhost:4444';

test.describe('Settings Modal Roundtrip', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(APP_URL, { waitUntil: 'networkidle', timeout: 15000 });
    test.skip(
      await page.locator('[data-testid="onboarding"]').isVisible(),
      'App in onboarding state'
    );
  });

  test('settings opens via header button click', async ({ page }) => {
    const settingsButton = page.locator('button[aria-label*="settings" i]')
      .or(page.getByRole('button', { name: /settings|preferences|gear/i }))
      .or(page.locator('[data-testid="settings-button"]'));
    await expect(settingsButton.first()).toBeVisible({ timeout: 10000 });

    await settingsButton.first().click();

    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible({ timeout: 3000 });
  });

  test('all settings tabs are present and visible', async ({ page }) => {
    // Open settings
    await page.keyboard.press(',');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible({ timeout: 3000 });

    // Look for tab elements within the modal
    const tabs = modal.getByRole('tab');
    const tabCount = await tabs.count();
    expect(tabCount).toBeGreaterThanOrEqual(2); // At least 2 tabs expected
  });

  test('settings tabs are navigable via click', async ({ page }) => {
    await page.keyboard.press(',');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible({ timeout: 3000 });

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
    await expect(modal).toBeVisible({ timeout: 3000 });

    // Find close button — could be X button or explicit close
    const closeButton = modal.getByRole('button', { name: /close|dismiss/i })
      .or(modal.locator('button[aria-label*="close" i]'))
      .or(modal.locator('[data-testid="close-button"]'));
    await closeButton.first().click();

    await expect(modal).not.toBeVisible({ timeout: 3000 });
  });

  test('settings closes via Escape key', async ({ page }) => {
    await page.keyboard.press(',');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible({ timeout: 3000 });

    await page.keyboard.press('Escape');
    await expect(modal).not.toBeVisible({ timeout: 3000 });
  });

  test('toggle switches are interactive', async ({ page }) => {
    await page.keyboard.press(',');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible({ timeout: 3000 });

    // Find toggle switches or checkboxes
    const toggles = modal.getByRole('switch').or(modal.getByRole('checkbox'));
    const toggleCount = await toggles.count();
    test.skip(toggleCount === 0, 'No toggles found in settings');

    const firstToggle = toggles.first();
    const initialState = await firstToggle.isChecked();

    await firstToggle.click();
    const newState = await firstToggle.isChecked();
    expect(newState).not.toBe(initialState);

    // Toggle back to restore original state
    await firstToggle.click();
    const restoredState = await firstToggle.isChecked();
    expect(restoredState).toBe(initialState);
  });

  test('About tab shows app information', async ({ page }) => {
    await page.keyboard.press(',');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible({ timeout: 3000 });

    // Navigate to About tab
    const aboutTab = modal.getByRole('tab', { name: /about/i })
      .or(modal.getByText(/about/i));
    const hasAbout = await aboutTab.isVisible().catch(() => false);
    test.skip(!hasAbout, 'No About tab found');

    await aboutTab.first().click();

    // About should show app name and version
    const appName = modal.getByText(/4da/i);
    await expect(appName.first()).toBeVisible();
  });

  test('settings can be reopened after closing', async ({ page }) => {
    // First open
    await page.keyboard.press(',');
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible({ timeout: 3000 });

    // Close
    await page.keyboard.press('Escape');
    await expect(modal).not.toBeVisible({ timeout: 3000 });

    // Reopen
    await page.keyboard.press(',');
    await expect(modal).toBeVisible({ timeout: 3000 });

    // Should still be functional — tabs should render
    const tabs = modal.getByRole('tab');
    const tabCount = await tabs.count();
    expect(tabCount).toBeGreaterThanOrEqual(1);

    // Clean up
    await page.keyboard.press('Escape');
  });
});
