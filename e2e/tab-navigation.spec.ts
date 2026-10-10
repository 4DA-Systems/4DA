// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Tab Navigation E2E Test
 *
 * Regression guard for the 2026-04-11 bug where Preemption and Blind Spots
 * tabs (Blind Spots is a Preemption sub-view since AD-054) were VISIBLE in the navbar but clicking them silently failed because
 * ui-slice.ts's TIER_VIEWS didn't match ViewTabBar.tsx's TIER_VIEWS.
 *
 * This test catches the exact class of bug: a tab renders in the DOM but
 * clicking it doesn't actually navigate. Smoke tests and unit tests won't
 * find this — only a real browser click against a real store.
 */

import { test, expect } from '@playwright/test';

import { MAIN_TABS, PREEMPTION_SUB_VIEWS, crashFallbacks, preemptionTablist } from './support/app';

// Relative to playwright.config's baseURL, so a run on another port tests its own server.
const APP_URL = '/';

test.describe('Tab navigation', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(APP_URL, { waitUntil: 'domcontentloaded', timeout: 15000 });
    test.skip(
      await page.locator('[data-testid="onboarding"]').isVisible().catch(() => false),
      'App in onboarding state',
    );
    // Wait for the tab bar to render. If the splash/signal-terminal takes the
    // screen, skip — this test only runs when the React UI is fully mounted.
    const tablist = page.getByRole('tablist', { name: /content views/i });
    const tablistVisible = await tablist.waitFor({ timeout: 15000 }).then(() => true).catch(() => false);
    test.skip(!tablistVisible, 'Tablist not rendered — app may be in cold-boot state');
  });

  // Every visible tab must be clickable and change aria-selected state.
  // If this fails for a tab, either TIER_VIEWS has drifted OR the tab is
  // rendered but setActiveView is rejecting it.
  //
  // The ids are ViewTabBar.tsx's TABS (main nav is three tabs, AD-054), and
  // each tab is located by the `id="tab-<id>"` it renders. The previous
  // locator filtered on the text `nav.<id>` — an i18n KEY, never the rendered
  // label ("Brief", "Blind Spots", ...) — so it matched nothing, every case hit
  // the "not visible" skip, and the suite could not fail. (CodeQL
  // js/identity-replacement flagged the `.replace('blindspots', 'blindspots')`
  // no-op inside that dead filter.)
  const ALL_NAVIGABLE_TABS = MAIN_TABS.map((t) => t.id);

  for (const tabId of ALL_NAVIGABLE_TABS) {
    test(`clicking tab "${tabId}" changes selection`, async ({ page }) => {
      const tab = page.getByRole('tablist', { name: /content views/i }).locator(`#tab-${tabId}`);

      // If the tab isn't visible (user on a lower tier that doesn't include
      // this view), skip — that's a valid state.
      const visible = await tab.isVisible().catch(() => false);
      test.skip(!visible, `Tab "${tabId}" not visible at current tier`);

      // Click the tab and assert it becomes selected
      await tab.click();
      await expect(tab).toHaveAttribute('aria-selected', 'true', { timeout: 3000 });
    });
  }

  test('preemption tab renders its view without error overlay', async ({ page }) => {
    const preemptionTab = page.getByRole('tab').filter({ hasText: /preemption/i }).first();
    const visible = await preemptionTab.isVisible().catch(() => false);
    test.skip(!visible, 'Preemption tab not visible at current tier');

    await preemptionTab.click();
    await expect(preemptionTab).toHaveAttribute('aria-selected', 'true', { timeout: 3000 });

    // Wait for the lazy-loaded view to mount (any content appearing after
    // the Suspense fallback resolves).
    await page.waitForTimeout(500);

    // No Vite error overlay should be visible
    const errorOverlay = page.locator('vite-error-overlay');
    expect(await errorOverlay.count()).toBe(0);

    // The view mounted and no React error boundary replaced it.
    await expect(page.getByRole('tabpanel', { name: 'Preemption' }).getByRole('heading', { level: 2 })).toBeVisible();
    await expect(crashFallbacks(page)).toHaveCount(0);
  });

  // AD-054: Blind Spots and Knowledge Gaps are Preemption sub-views. Every
  // sub-tab must select, mount its lazy view and survive the backendless IPC
  // failures without an error boundary.
  for (const { id, label } of PREEMPTION_SUB_VIEWS) {
    test(`preemption sub-view "${id}" selects and renders without error`, async ({ page }) => {
      await page.getByRole('tablist', { name: /content views/i }).locator('#tab-preemption').click();
      const subTab = preemptionTablist(page).locator(`#preemption-tab-${id}`);
      await expect(subTab).toContainText(label);
      await subTab.click();
      await expect(subTab).toHaveAttribute('aria-selected', 'true', { timeout: 3000 });
      const subPanel = page.locator(`#preemption-panel-${id}`);
      await expect(subPanel).toBeVisible();
      await expect(subPanel).toHaveAttribute('aria-labelledby', `preemption-tab-${id}`);
      await page.waitForTimeout(500);
      expect(await page.locator('vite-error-overlay').count()).toBe(0);
      await expect(crashFallbacks(page)).toHaveCount(0);
    });
  }

  test('preemption sub-tabs use a roving tabindex driven by the arrow keys', async ({ page }) => {
    await page.getByRole('tablist', { name: /content views/i }).locator('#tab-preemption').click();
    const list = preemptionTablist(page);
    const worklist = list.locator('#preemption-tab-worklist');
    await expect(worklist).toHaveAttribute('aria-selected', 'true');
    await expect(list.locator('[role="tab"][tabindex="0"]')).toHaveCount(1);
    await worklist.focus();
    await page.keyboard.press('ArrowRight');
    const blindSpots = list.locator('#preemption-tab-blindspots');
    await expect(blindSpots).toBeFocused();
    await expect(blindSpots).toHaveAttribute('aria-selected', 'true');
    await expect(blindSpots).toHaveAttribute('tabindex', '0');
    await page.keyboard.press('End');
    await expect(list.locator('#preemption-tab-knowledge')).toBeFocused();
  });
});
