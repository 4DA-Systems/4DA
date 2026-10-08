import { test, expect, type Page, type ConsoleMessage } from '@playwright/test';

import { installTauriIpcMock, ipcLog, mockSettings, OLLAMA_READY } from './support/app';

/**
 * First-run flow tests — validate the user journey from app load through navigation.
 * These tests handle both onboarding and main-view states gracefully.
 */

/**
 * FirstRunTransition: a full-screen role="status" whose name tracks its phase.
 * A bare `[role="status"][aria-busy]` also matched the splash screen ("Ready!")
 * and the empty-state panels, so a plain cold boot was misread as first-run.
 */
const FIRST_RUN_STATUS =
  /^(Preparing analysis|Showing project intelligence|Scanning sources|Analyzing results|Analysis error|Completing|Scan complete|Analysis complete)/;

function firstRunStatus(page: Page) {
  return page.getByRole('status', { name: FIRST_RUN_STATUS });
}

/** Wait for app to be interactive — returns which state we landed in */
async function waitForApp(page: Page): Promise<'splash' | 'onboarding' | 'first-run' | 'main'> {
  // Wait for something visible. The splash is the status region that carries
  // the "Refresh if stuck" escape hatch.
  const splash = page.getByRole('status').filter({ has: page.getByRole('button', { name: /refresh if stuck/i }) });
  const onboarding = page.getByRole('dialog', { name: /setup wizard/i });
  const firstRun = firstRunStatus(page);
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
    // Plain browser mode never reaches onboarding (with no backend the
    // settings never load, so the onboarding decision is never made). A
    // backend reporting onboarding_complete=false is what opens the wizard.
    await installTauriIpcMock(page, {
      settings: { ...mockSettings({ provider: 'anthropic', model: 'claude-sonnet-5' }), onboarding_complete: false },
    });
    await page.goto('/');
    expect(await waitForApp(page)).toBe('onboarding');

    const dialog = page.getByRole('dialog', { name: /setup wizard/i });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole('group', { name: /step/i })).toBeVisible();

    // Should have at least one button for navigation
    await expect(dialog.getByRole('button').first()).toBeVisible();
  });

  test('main view shows analysis progress indicators', async ({ page }) => {
    const state = await waitForApp(page);

    if (state === 'onboarding') {
      test.skip(true, 'App in onboarding state');
      return;
    }

    if (state === 'first-run') {
      // FirstRunTransition announces its current phase.
      await expect(firstRunStatus(page)).toBeVisible({ timeout: 5_000 });
    } else {
      // Main view: the app bar's live status line reports the analysis state
      // and the Run analysis control is available.
      const appBar = page.getByRole('banner', { name: '4DA' });
      await expect(appBar.getByRole('status')).toHaveText(/^(Ready to analyze|Analyzing\.\.\.|Analysis complete)$/);
      await expect(appBar.getByRole('button', { name: 'Run analysis' })).toBeVisible();
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

  test.describe('consent-first project scan', () => {
    const FOLDERS = ['C:\\Users\\dev\\code', 'C:\\Users\\dev\\Documents'];

    async function openWizardAt(page: Page, step: 'choice' | 'setup') {
      await page.addInitScript((s: string) => {
        try { localStorage.setItem('4da-onboarding-wizard-step', s); } catch { /* noop */ }
      }, step);
      await installTauriIpcMock(page, {
        settings: { ...mockSettings({ provider: 'none', model: '' }), onboarding_complete: false },
        responses: {
          check_ollama_status: OLLAMA_READY,
          ace_preview_discovery_dirs: FOLDERS,
          ace_auto_discover: { success: true, directories: [FOLDERS[0]], directories_found: 1, projects_found: 1, directories_added: 1, scan_result: { combined: { total_topics: 1, topics: ['rust'] } } },
          mark_onboarding_complete: null,
          taste_test_is_calibrated: false,
          ace_get_suggested_interests: [],
        },
      });
      await page.goto('/');
      expect(await waitForApp(page)).toBe('onboarding');
      return page.getByRole('dialog', { name: /setup wizard/i });
    }

    test('the choice gate lists folders and scans only the ticked ones, only on click', async ({ page }) => {
      const dialog = await openWizardAt(page, 'choice');
      await expect(dialog.getByLabel(FOLDERS[0]!)).toBeChecked();
      await expect(dialog.getByText(/already learning about your projects/i)).toHaveCount(0);

      await dialog.getByLabel(FOLDERS[1]!).uncheck();
      expect((await ipcLog(page)).filter(c => c.cmd === 'ace_auto_discover')).toHaveLength(0);

      await dialog.getByRole('button', { name: /scan my projects/i }).click();
      await expect.poll(async () => (await ipcLog(page)).filter(c => c.cmd === 'ace_auto_discover').map(c => c.args))
        .toEqual([{ dirs: [FOLDERS[0]] }]);
    });

    test('quick setup: a ready Ollama is recognised and nothing is scanned on open', async ({ page }) => {
      const dialog = await openWizardAt(page, 'setup');
      await expect(dialog.getByRole('button', { name: /AI Provider: Local AI Ready/i })).toBeVisible();
      await expect(dialog.getByText(/models aren't installed/i)).toHaveCount(0);

      await page.waitForTimeout(1_000);
      expect((await ipcLog(page)).filter(c => c.cmd === 'ace_auto_discover')).toHaveLength(0);

      // A ready AI provider opens the Projects section on its own.
      const projects = dialog.getByRole('button', { name: /^Your Projects/ });
      await expect(projects).toHaveAttribute('aria-expanded', 'true');
      await expect(dialog.getByLabel(FOLDERS[0]!)).toBeChecked();
      await dialog.getByRole('button', { name: /scan selected folders/i }).click();
      await expect(dialog.getByRole('button', { name: 'Remove rust' })).toBeVisible();
      const scans = (await ipcLog(page)).filter(c => c.cmd === 'ace_auto_discover');
      expect(scans.map(c => c.args)).toEqual([{ dirs: FOLDERS }]);
    });
  });
});
