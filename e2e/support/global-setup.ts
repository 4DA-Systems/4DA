// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { chromium, type FullConfig, type Page } from '@playwright/test';

import { MAIN_TABS, mainTablist } from './app';

/**
 * Warm the E2E Vite server before the first test runs.
 *
 * Playwright's `webServer.url` check passes as soon as Vite serves
 * `index.html`, but a cold Vite has not yet pre-bundled dependencies or
 * transformed the app's module graph — that happens on the first browser load,
 * and lazy chunks (each main view, the Settings modal) are transformed only
 * when first imported. The first test therefore paid the whole cold start
 * inside its own budget: `analysis-flow.spec.ts` "app bar is a labelled banner
 * landmark" timed out in `gotoMainShell` (20 s), and the first Settings-modal
 * open in `settings-roundtrip.spec.ts` blew its 3 s dialog bound. Vite may
 * also discover a late dependency, re-optimise, and force a full page reload
 * mid-test.
 *
 * So: load the shell until it comes up within the same 20 s budget the tests
 * use (a re-optimise reload just costs another round), then import every lazy
 * view and the Settings modal once, so no test is the first to compile them.
 */
const SHELL_BUDGET_MS = 20_000; // same bound as gotoMainShell
const WARMUP_DEADLINE_MS = 240_000;

async function shellLoadsWithinBudget(page: Page, baseURL: string): Promise<boolean> {
  try {
    await page.goto(baseURL, { timeout: 60_000 });
    await mainTablist(page).waitFor({ state: 'visible', timeout: SHELL_BUDGET_MS });
    return true;
  } catch {
    return false;
  }
}

async function importLazyChunks(page: Page): Promise<void> {
  for (const { label } of MAIN_TABS) {
    const tab = mainTablist(page).getByRole('tab', { name: label });
    await tab.click({ timeout: 10_000 });
    await page.waitForLoadState('networkidle', { timeout: 30_000 });
  }
  await page.keyboard.press(',');
  await page.getByRole('dialog').first().waitFor({ state: 'visible', timeout: 30_000 });
  await page.waitForLoadState('networkidle', { timeout: 30_000 });
  await page.keyboard.press('Escape');
}

export default async function globalSetup(config: FullConfig): Promise<void> {
  const baseURL = config.projects[0]?.use.baseURL;
  if (!baseURL) throw new Error('[e2e warm-up] no baseURL configured');

  const started = Date.now();
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    let rounds = 0;
    // Two consecutive in-budget loads: the second proves a dependency
    // re-optimise reload did not land after the first.
    let consecutive = 0;
    while (consecutive < 2) {
      if (Date.now() - started > WARMUP_DEADLINE_MS) {
        throw new Error(
          `[e2e warm-up] the app shell did not load within ${SHELL_BUDGET_MS} ms after ` +
            `${Math.round((Date.now() - started) / 1000)} s of warming (${rounds} rounds)`,
        );
      }
      rounds += 1;
      consecutive = (await shellLoadsWithinBudget(page, baseURL)) ? consecutive + 1 : 0;
    }

    try {
      await importLazyChunks(page);
      // Lazy imports can surface a late dependency; confirm the shell still
      // loads in budget after them.
      if (!(await shellLoadsWithinBudget(page, baseURL))) {
        await shellLoadsWithinBudget(page, baseURL);
      }
    } catch (e) {
      // Best effort: the shell is warm, which is the part every test needs.
      console.warn(`[e2e warm-up] lazy-chunk warm-up incomplete: ${String(e)}`);
    }
    console.log(
      `[e2e warm-up] Vite warm in ${Math.round((Date.now() - started) / 1000)} s (${rounds} shell rounds)`,
    );
  } finally {
    await browser.close();
  }
}
