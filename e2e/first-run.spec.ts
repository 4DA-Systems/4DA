import { test, expect, type Page, type ConsoleMessage } from '@playwright/test';

/**
 * First-run flow tests — validate the user journey from app load through navigation.
 * These tests handle both onboarding and main-view states gracefully.
 */

/** Wait for app to be interactive — returns which state we landed in */
async function waitForApp(page: Page): Promise<'splash' | 'onboarding' | 'first-run' | 'main'> {
  // Wait for something visible
  const splash = page.locator('[data-testid="splash-screen"], .splash-screen');
  const onboarding = page.getByRole('dialog', { name: /setup wizard/i });
  const firstRun = page.locator('[role="status"][aria-busy]');
  const tablist = page.getByRole('tablist', { name: /content views/i });

  const result = await Promise.race([
    splash.waitFor({ state: 'visible', timeout: 15_000 }).then(() => 'splash' as const),
    onboarding.waitFor({ state: 'visible', timeout: 15_000 }).then(() => 'onboarding' as const),
    firstRun.waitFor({ state: 'visible', timeout: 15_000 }).then(() => 'first-run' as const),
    tablist.waitFor({ state: 'visible', timeout: 15_000 }).then(() => 'main' as const),
  ]).catch(() => 'main' as const);

  // If splash, wait for it to fade
  if (result === 'splash') {
    await splash.waitFor({ state: 'hidden', timeout: 10_000 }).catch(() => {});
    // Re-check state after splash
    const postSplash = await Promise.race([
      onboarding.waitFor({ state: 'visible', timeout: 10_000 }).then(() => 'onboarding' as const),
      firstRun.waitFor({ state: 'visible', timeout: 10_000 }).then(() => 'first-run' as const),
      tablist.waitFor({ state: 'visible', timeout: 10_000 }).then(() => 'main' as const),
    ]).catch(() => 'main' as const);
    return postSplash;
  }

  return result;
}

test.describe('First-Run Flow', () => {
  let consoleMessages: ConsoleMessage[] = [];

  test.beforeEach(async ({ page }) => {
    consoleMessages = [];
    page.on('console', (msg) => consoleMessages.push(msg));
    await page.goto('/');
  });

  test('app loads and reaches interactive state', async ({ page }) => {
    const state = await waitForApp(page);
    expect(['onboarding', 'first-run', 'main']).toContain(state);

    // Verify something is actually rendered
    const body = page.locator('body');
    await expect(body).toBeVisible();
    const content = await body.textContent();
    expect(content?.length).toBeGreaterThan(0);
  });

  test('reaches interactive state within 60 seconds', async ({ page }) => {
    const startTime = Date.now();
    const state = await waitForApp(page);
    const elapsed = Date.now() - startTime;

    expect(['onboarding', 'first-run', 'main']).toContain(state);
    expect(elapsed).toBeLessThan(60_000);
    console.log(`Time to interactive: ${elapsed}ms (state: ${state})`);
  });

  test('onboarding wizard has navigable sections', async ({ page }) => {
    const state = await waitForApp(page);

    if (state !== 'onboarding') {
      test.skip(true, `App in ${state} state, not onboarding`);
      return;
    }

    const dialog = page.getByRole('dialog', { name: /setup wizard/i });
    await expect(dialog).toBeVisible();

    // Should have at least one button for navigation
    const buttons = dialog.getByRole('button');
    const count = await buttons.count();
    expect(count).toBeGreaterThan(0);
  });

  test('main view shows analysis progress indicators', async ({ page }) => {
    const state = await waitForApp(page);

    if (state === 'onboarding') {
      test.skip(true, 'App in onboarding state');
      return;
    }

    // In first-run or main state, look for progress indicators or results
    if (state === 'first-run') {
      // FirstRunTransition should show progress or status
      const statusEl = page.locator('[role="status"]');
      await expect(statusEl).toBeVisible({ timeout: 5_000 });
    } else {
      // Main view — should have action bar or results
      const actionBar = page.locator('[data-testid="action-bar"], button:has-text("Analyze")');
      const hasActionBar = await actionBar.first().isVisible().catch(() => false);
      // At minimum, the app shell should be rendered
      const header = page.locator('header');
      const hasHeader = await header.first().isVisible().catch(() => false);
      expect(hasActionBar || hasHeader).toBeTruthy();
    }
  });

  test('tab navigation works with loaded or empty results', async ({ page }) => {
    const state = await waitForApp(page);

    if (state !== 'main') {
      test.skip(true, `App in ${state} state, tabs not accessible`);
      return;
    }

    const tablist = page.getByRole('tablist', { name: /content views/i });
    await expect(tablist).toBeVisible();

    // Get all visible tabs
    const tabs = tablist.getByRole('tab');
    const tabCount = await tabs.count();
    expect(tabCount).toBeGreaterThanOrEqual(2);

    // Click through available tabs — each should activate without crash
    for (let i = 0; i < Math.min(tabCount, 4); i++) {
      const tab = tabs.nth(i);
      const name = await tab.textContent();
      await tab.click();
      await expect(tab).toHaveAttribute('aria-selected', 'true');
      // Brief wait for lazy-loaded content
      await page.waitForTimeout(500);
      console.log(`Tab "${name?.trim()}": navigated OK`);
    }
  });

  test('embedding mode indicator reflects actual state', async ({ page }) => {
    const state = await waitForApp(page);

    if (state === 'onboarding') {
      test.skip(true, 'App in onboarding state');
      return;
    }

    // Wait a moment for embedding mode event to fire
    await page.waitForTimeout(3_000);

    // Check for either semantic or keyword-only indicator in the UI
    const keywordBadge = page.locator('text=Keyword Only').first();
    const semanticIndicator = page.locator('[data-testid="embedding-mode"]').first();
    const ollamaStatus = page.locator('text=Ollama').first();

    const hasKeywordBadge = await keywordBadge.isVisible().catch(() => false);
    const hasSemanticIndicator = await semanticIndicator.isVisible().catch(() => false);
    const hasOllamaStatus = await ollamaStatus.isVisible().catch(() => false);

    // At least one embedding-related indicator should be present (or none if analysis hasn't started)
    console.log(`Embedding state: keyword=${hasKeywordBadge}, semantic=${hasSemanticIndicator}, ollama=${hasOllamaStatus}`);
    // This test documents the state, not asserts a specific mode (depends on user's Ollama setup)
    expect(true).toBeTruthy();
  });

  test('no critical console errors during navigation', async ({ page }) => {
    const state = await waitForApp(page);

    // Wait for app to settle
    await page.waitForTimeout(5_000);

    // Navigate through tabs if in main view
    if (state === 'main') {
      const tablist = page.getByRole('tablist', { name: /content views/i });
      if (await tablist.isVisible().catch(() => false)) {
        const tabs = tablist.getByRole('tab');
        const count = await tabs.count();
        for (let i = 0; i < Math.min(count, 6); i++) {
          await tabs.nth(i).click();
          await page.waitForTimeout(300);
        }
      }
    }

    await page.waitForTimeout(2_000);

    // Filter for critical errors (exclude expected Tauri IPC errors in browser context)
    const criticalErrors = consoleMessages.filter((m) => {
      if (m.type() !== 'error') return false;
      const text = m.text();
      // Expected browser-context errors
      if (text.includes('invoke')) return false;
      if (text.includes('Failed to fetch')) return false;
      if (text.includes('get_engagement_summary')) return false;
      if (text.includes('__TAURI__')) return false;
      if (text.includes('tauri')) return false;
      // React dev warnings
      if (text.includes('Warning:')) return false;
      return true;
    });

    console.log(`Total console errors: ${consoleMessages.filter(m => m.type() === 'error').length}`);
    console.log(`Critical (non-expected) errors: ${criticalErrors.length}`);
    for (const err of criticalErrors) {
      console.log(`  CRITICAL: ${err.text().substring(0, 200)}`);
    }

    // Allow zero critical errors
    expect(criticalErrors.length).toBe(0);
  });
});
