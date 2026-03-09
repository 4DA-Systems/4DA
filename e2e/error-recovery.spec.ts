import { test, expect } from '@playwright/test';

const APP_URL = 'http://localhost:4444';

// Tauri IPC errors that are expected and should not fail tests
const EXPECTED_ERROR_PATTERNS = [
  /tauri/i,
  /ipc/i,
  /__TAURI__/,
  /Failed to fetch/,
  /NetworkError/,
  /ResizeObserver loop/,
];

function isExpectedError(message: string): boolean {
  return EXPECTED_ERROR_PATTERNS.some((pattern) => pattern.test(message));
}

test.describe('Error Recovery & Resilience', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(APP_URL, { waitUntil: 'networkidle', timeout: 15000 });
    test.skip(
      await page.locator('[data-testid="onboarding"]').isVisible(),
      'App in onboarding state'
    );
  });

  test('tabs render without triggering error boundary', async ({ page }) => {
    const tablist = page.getByRole('tablist');
    const hasTablist = await tablist.isVisible().catch(() => false);
    test.skip(!hasTablist, 'No tab list visible');

    // No error boundary fallback should be visible
    const errorBoundary = page.locator('[data-testid="error-boundary"]')
      .or(page.getByText(/something went wrong/i))
      .or(page.getByText(/error occurred/i));
    const hasError = await errorBoundary.isVisible().catch(() => false);
    expect(hasError).toBe(false);

    // Tabs should be present and functional
    const tabs = tablist.getByRole('tab');
    const tabCount = await tabs.count();
    expect(tabCount).toBeGreaterThanOrEqual(1);
  });

  test('rapid tab switching does not crash', async ({ page }) => {
    const tablist = page.getByRole('tablist');
    const hasTablist = await tablist.isVisible().catch(() => false);
    test.skip(!hasTablist, 'No tab list visible');

    const tabs = tablist.getByRole('tab');
    const tabCount = await tabs.count();
    test.skip(tabCount < 2, 'Not enough tabs for switching test');

    // Rapidly switch between tabs 20 times
    for (let i = 0; i < 20; i++) {
      await tabs.nth(i % tabCount).click();
      // Minimal delay to simulate rapid human clicking
      await page.waitForTimeout(50);
    }

    // App should still be responsive — no crash, no error boundary
    const errorBoundary = page.getByText(/something went wrong/i)
      .or(page.getByText(/error occurred/i));
    const hasError = await errorBoundary.isVisible().catch(() => false);
    expect(hasError).toBe(false);

    // Tabs should still be functional
    await expect(tablist).toBeVisible();
  });

  test('rapid modal open/close does not crash', async ({ page }) => {
    // Rapidly open and close settings modal 10 times
    for (let i = 0; i < 10; i++) {
      await page.keyboard.press(',');
      await page.waitForTimeout(100);
      await page.keyboard.press('Escape');
      await page.waitForTimeout(100);
    }

    // App should still be responsive
    const body = page.locator('body');
    await expect(body).toBeVisible();

    // No leftover modal should be open
    const modal = page.getByRole('dialog');
    const modalVisible = await modal.isVisible().catch(() => false);
    expect(modalVisible).toBe(false);
  });

  test('empty search input does not cause errors', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error' && !isExpectedError(msg.text())) {
        consoleErrors.push(msg.text());
      }
    });

    const searchInput = page.getByRole('searchbox')
      .or(page.getByPlaceholder(/search|filter|find/i))
      .or(page.locator('input[type="search"]'));
    const hasSearch = await searchInput.isVisible().catch(() => false);
    test.skip(!hasSearch, 'No search input visible');

    // Submit empty search
    await searchInput.first().focus();
    await page.keyboard.press('Enter');
    await page.waitForTimeout(500);

    // Type and clear rapidly
    await searchInput.first().fill('test');
    await searchInput.first().fill('');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(500);

    // No unexpected console errors should have occurred
    expect(consoleErrors).toHaveLength(0);
  });

  test('no critical console errors on page load', async ({ page }) => {
    const criticalErrors: string[] = [];

    // Set up error collection on a fresh page load
    page.on('console', (msg) => {
      if (msg.type() === 'error' && !isExpectedError(msg.text())) {
        criticalErrors.push(msg.text());
      }
    });

    page.on('pageerror', (error) => {
      if (!isExpectedError(error.message)) {
        criticalErrors.push(`Page error: ${error.message}`);
      }
    });

    // Reload to capture errors from a full page load
    await page.reload({ waitUntil: 'networkidle', timeout: 15000 });
    await page.waitForTimeout(2000); // Allow async operations to settle

    // Filter out any remaining expected errors
    const unexpectedErrors = criticalErrors.filter(
      (err) => !isExpectedError(err)
    );

    if (unexpectedErrors.length > 0) {
      console.log('Unexpected console errors:', unexpectedErrors);
    }
    expect(unexpectedErrors).toHaveLength(0);
  });

  test('page does not show uncaught exception overlay', async ({ page }) => {
    // Vite/React dev mode shows error overlays for uncaught exceptions
    const errorOverlay = page.locator('vite-error-overlay')
      .or(page.locator('#react-error-overlay'))
      .or(page.locator('[data-testid="error-overlay"]'));

    const hasOverlay = await errorOverlay.isVisible().catch(() => false);
    expect(hasOverlay).toBe(false);
  });
});
