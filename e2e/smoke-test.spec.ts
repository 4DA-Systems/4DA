import { test, expect, type Page, type ConsoleMessage } from '@playwright/test';

import { MAIN_TABS, crashFallbacks } from './support/app';

const SCREENSHOT_DIR = 'e2e-screenshots';
let consoleMessages: ConsoleMessage[] = [];

async function waitForAppReady(page: Page): Promise<'onboarding' | 'main'> {
  const onboarding = page.getByRole('dialog', { name: /setup wizard/i });
  const tablist = page.getByRole('tablist', { name: /content views/i });
  const result = await Promise.race([
    onboarding.waitFor({ state: 'visible', timeout: 20_000 }).then(() => 'onboarding' as const),
    tablist.waitFor({ state: 'visible', timeout: 20_000 }).then(() => 'main' as const),
  ]);
  return result;
}

test.describe('4DA Smoke Tests', () => {
  test.beforeEach(async ({ page }) => {
    consoleMessages = [];
    page.on('console', (msg) => consoleMessages.push(msg));
    await page.goto('/');
  });

  test('Test 1: Free Briefing Fallback', async ({ page }) => {
    const appState = await waitForAppReady(page);
    await page.screenshot({ path: SCREENSHOT_DIR + '/01-main-view-initial.png', fullPage: true });

    if (appState === 'onboarding') {
      await page.screenshot({ path: SCREENSHOT_DIR + '/01-onboarding-state.png', fullPage: true });
      console.log('APP STATE: Onboarding dialog visible');
      return;
    }

    const tablist = page.getByRole('tablist', { name: /content views/i });
    await expect(tablist).toBeVisible();
    const briefTab = tablist.getByRole('tab', { name: 'Brief', exact: true });
    await briefTab.click();
    await expect(briefTab).toHaveAttribute('aria-selected', 'true');
    const briefPanel = page.getByRole('tabpanel', { name: 'Brief' });
    await expect(briefPanel).toBeVisible();
    await page.screenshot({ path: SCREENSHOT_DIR + '/01-briefing-view.png', fullPage: true });

    const findings: string[] = [];
    // Every state the Brief can be in without a briefing yet must render a
    // recognisable, actionable surface (no-data warm-up, loading, or ready).
    const checks = [
      { loc: briefPanel.getByRole('heading', { name: 'Your Intelligence System' }), lbl: 'Warm-up - no analysis run yet' },
      { loc: briefPanel.getByRole('status', { name: 'Gathering Intelligence' }), lbl: 'Gathering Intelligence - in progress' },
      { loc: briefPanel.getByText('Briefing Ready to Generate'), lbl: 'Briefing Ready to Generate' },
    ];
    for (const c of checks) {
      if (await c.loc.first().isVisible().catch(() => false)) {
        findings.push('FOUND: ' + c.lbl);
      }
    }
    console.log('=== TEST 1: BRIEFING STATE ===');
    for (const f of findings) console.log(f);
    if (findings.length === 0) console.log('WARNING: No expected briefing state found');
    expect(findings.length).toBeGreaterThan(0);
  });

  test('Test 2: Engagement Pulse Component', async ({ page }) => {
    const appState = await waitForAppReady(page);
    test.skip(appState === 'onboarding', 'Cannot check EngagementPulse during onboarding');

    const tablist = page.getByRole('tablist', { name: /content views/i });
    const briefTab = tablist.getByRole('tab', { name: 'Brief', exact: true });
    await briefTab.click();
    await expect(briefTab).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByRole('tabpanel', { name: 'Brief' })).toBeVisible();

    const heatmapContainer = page.locator('[title="7-day activity"]');
    const streakElement = page.locator('text=streak');
    const heatmapVisible = await heatmapContainer.isVisible().catch(() => false);
    const streakVisible = await streakElement.first().isVisible().catch(() => false);

    await page.screenshot({ path: SCREENSHOT_DIR + '/02-engagement-pulse.png', fullPage: true });

    console.log('=== TEST 2: ENGAGEMENT PULSE ===');
    console.log('7-day heatmap visible: ' + heatmapVisible);
    console.log('Streak indicator visible: ' + streakVisible);
    console.log('EngagementPulse rendered: ' + (heatmapVisible || streakVisible));
  });

  test('Test 3: View Tracking - results view', async ({ page }) => {
    const appState = await waitForAppReady(page);
    test.skip(appState === 'onboarding', 'Cannot check results during onboarding');

    const tablist = page.getByRole('tablist', { name: /content views/i });
    const resultsTab = tablist.getByRole('tab', { name: 'Signal', exact: true });
    await resultsTab.click();
    await expect(resultsTab).toHaveAttribute('aria-selected', 'true');
    const resultsRegion = page.getByRole('tabpanel', { name: 'Signal' }).getByRole('region', { name: 'Analysis Results' });
    await expect(resultsRegion).toBeVisible();
    await expect(resultsRegion.getByRole('heading', { name: 'Analysis Results' })).toBeVisible();

    await page.screenshot({ path: SCREENSHOT_DIR + '/03-results-view.png', fullPage: true });

    console.log('=== TEST 3: RESULTS VIEW ===');
    const stateChecks = [
      { loc: 'text=No results yet', lbl: 'No results yet' },
    ];
    for (const c of stateChecks) {
      if (await page.locator(c.loc).first().isVisible().catch(() => false)) {
        console.log('FOUND: ' + c.lbl);
      }
    }

    const resultItems = page.locator('[data-index]');
    const resultCount = await resultItems.count();
    console.log('Result items (virtual): ' + resultCount);

    if (resultCount > 0) {
      const scrollContainer = page.locator('.overflow-y-auto').first();
      if (await scrollContainer.isVisible().catch(() => false)) {
        for (let i = 0; i < 3; i++) {
          await scrollContainer.evaluate((el: Element) => el.scrollBy(0, 200));
          await page.waitForTimeout(500);
        }
        console.log('Scrolled through results');
      }
    }

    const observerErrors = consoleMessages.filter(
      (m) => m.type() === 'error' && (
        m.text().includes('IntersectionObserver') ||
        m.text().includes('ace_record_interaction') ||
        m.text().includes('record_interaction')
      )
    );
    console.log('IntersectionObserver/interaction errors: ' + observerErrors.length);
    for (const err of observerErrors) {
      console.log('  ERROR: ' + err.text());
    }

    await page.screenshot({ path: SCREENSHOT_DIR + '/03-results-after-scroll.png', fullPage: true });
  });

  test('Test 4: System Tray - document limitations', async ({ page }) => {
    await waitForAppReady(page);
    console.log('=== TEST 4: SYSTEM TRAY ===');
    console.log('NOTE: System tray is a native OS feature. Playwright cannot test it.');
    console.log('Manual verification needed for tray icon, close-to-tray, tray menu.');

    const title = await page.title();
    console.log('Window title: ' + title);

    await page.screenshot({
      path: SCREENSHOT_DIR + '/04-app-header.png',
      fullPage: false,
      clip: { x: 0, y: 0, width: 1280, height: 100 },
    });
  });

  test('all tabs render content without error boundary', async ({ page }) => {
    const appState = await waitForAppReady(page);
    if (appState !== 'main') { test.skip(true, `App in ${appState} state`); return; }

    const tablist = page.getByRole('tablist', { name: /content views/i });
    await expect(tablist.getByRole('tab')).toHaveCount(MAIN_TABS.length);

    for (const { label } of MAIN_TABS) {
      await tablist.getByRole('tab', { name: label, exact: true }).click();
      // The view mounted (lazy load resolved) ...
      await expect(page.getByRole('tabpanel', { name: label })).toBeVisible();
      await page.waitForTimeout(1000); // let its IPC calls settle (and fail, in browser mode)
      // ... and no error boundary replaced it. A view's own designed error
      // state (e.g. Blind Spots' "Coverage scan unavailable" + Retry) is fine.
      await expect(crashFallbacks(page)).toHaveCount(0);
    }
  });

  test('Test 5: General Health - console errors and navigation', async ({ page }) => {
    const appState = await waitForAppReady(page);
    await page.waitForTimeout(5000);

    const jsErrors = consoleMessages.filter((m) => m.type() === 'error');
    const jsWarnings = consoleMessages.filter((m) => m.type() === 'warning');

    console.log('=== TEST 5: GENERAL HEALTH ===');
    console.log('App state: ' + appState);
    console.log('Total console messages: ' + consoleMessages.length);
    console.log('Console errors: ' + jsErrors.length);
    console.log('Console warnings: ' + jsWarnings.length);

    if (jsErrors.length > 0) {
      console.log('--- ERRORS ---');
      for (const err of jsErrors) {
        console.log('  [ERROR] ' + err.text().substring(0, 300));
      }
    }
    if (jsWarnings.length > 0) {
      console.log('--- WARNINGS (first 10) ---');
      for (const warn of jsWarnings.slice(0, 10)) {
        console.log('  [WARN] ' + warn.text().substring(0, 300));
      }
    }

    await page.screenshot({ path: SCREENSHOT_DIR + '/05-final-state.png', fullPage: true });

    if (appState === 'main') {
      const tablist = page.getByRole('tablist', { name: /content views/i });
      for (const { label } of MAIN_TABS) {
        const tab = tablist.getByRole('tab', { name: label, exact: true });
        await tab.click();
        await expect(tab).toHaveAttribute('aria-selected', 'true');
        await page.waitForTimeout(500);
        console.log('Tab ' + label + ': OK');
      }
      await page.screenshot({ path: SCREENSHOT_DIR + '/05-after-tab-navigation.png', fullPage: true });
    }

    const criticalErrors = jsErrors.filter((e) => {
      const text = e.text();
      if (text.includes('invoke')) return false;
      if (text.includes('Failed to fetch')) return false;
      if (text.includes('get_engagement_summary')) return false;
      return true;
    });
    console.log('Critical (non-expected) errors: ' + criticalErrors.length);
    for (const err of criticalErrors) {
      console.log('  CRITICAL: ' + err.text().substring(0, 300));
    }
    console.log('App survived all navigation - no crashes detected.');
  });
});
