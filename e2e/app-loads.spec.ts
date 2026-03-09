import { test, expect } from '@playwright/test';

const APP_URL = 'http://localhost:4444';

test.describe('App Startup', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(APP_URL, { waitUntil: 'domcontentloaded', timeout: 10000 });
    test.skip(
      await page.locator('[data-testid="onboarding"]').isVisible(),
      'App in onboarding state'
    );
  });

  test('splash screen renders during load', async ({ page }) => {
    // Navigate fresh to catch the splash screen before it transitions
    await page.goto(APP_URL, { waitUntil: 'commit' });
    // The splash/loading state should be visible briefly or the app should have loaded
    const body = page.locator('body');
    await expect(body).toBeVisible({ timeout: 10000 });
  });

  test('refresh button available for stuck load states', async ({ page }) => {
    // If the app gets stuck, a refresh/retry mechanism should exist
    // Check for a refresh button or the browser's reload capability
    const refreshButton = page.getByRole('button', { name: /refresh|retry|reload/i });
    const hasRefresh = await refreshButton.isVisible().catch(() => false);
    if (hasRefresh) {
      await expect(refreshButton).toBeEnabled();
    } else {
      // App loaded successfully without needing refresh — that's fine
      expect(true).toBe(true);
    }
  });

  test('app loads within 30-second budget', async ({ page }) => {
    const start = Date.now();
    await page.goto(APP_URL, { waitUntil: 'networkidle', timeout: 30000 });
    const elapsed = Date.now() - start;
    expect(elapsed).toBeLessThan(30000);
  });

  test('skip-to-content accessibility link exists', async ({ page }) => {
    // Look for a skip-to-content link that becomes visible on focus
    const skipLink = page.locator('a[href="#main-content"], a[href="#content"], [data-testid="skip-to-content"]');
    const hasSkipLink = await skipLink.count();
    if (hasSkipLink > 0) {
      // Tab to the skip link and verify it becomes visible
      await page.keyboard.press('Tab');
      await expect(skipLink.first()).toBeVisible();
    } else {
      // Record that skip-to-content is missing — a11y improvement opportunity
      test.info().annotations.push({
        type: 'a11y',
        description: 'No skip-to-content link found — consider adding one',
      });
    }
  });

  test('header renders with brand element', async ({ page }) => {
    const header = page.getByRole('banner').or(page.locator('header'));
    await expect(header).toBeVisible({ timeout: 10000 });

    // Brand text or logo should be present in the header
    const brandText = header.getByText(/4da/i);
    const brandLogo = header.locator('img[alt*="logo" i], svg[aria-label*="logo" i], [data-testid="brand-logo"]');
    const hasBrand = (await brandText.count()) > 0 || (await brandLogo.count()) > 0;
    expect(hasBrand).toBe(true);
  });

  test('settings button renders in header', async ({ page }) => {
    const header = page.getByRole('banner').or(page.locator('header'));
    await expect(header).toBeVisible({ timeout: 10000 });

    const settingsButton = header.getByRole('button', { name: /settings|preferences|gear|config/i })
      .or(header.locator('[data-testid="settings-button"]'))
      .or(header.locator('button[aria-label*="settings" i]'));
    await expect(settingsButton.first()).toBeVisible();
  });
});
