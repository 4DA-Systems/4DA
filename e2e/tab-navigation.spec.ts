// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Tab Navigation E2E Test
 *
 * Regression guard for the 2026-04-11 bug where Preemption and Blind Spots
 * tabs were VISIBLE in the navbar but clicking them silently failed because
 * ui-slice.ts's TIER_VIEWS didn't match ViewTabBar.tsx's TIER_VIEWS.
 *
 * This test catches the exact class of bug: a tab renders in the DOM but
 * clicking it doesn't actually navigate. Smoke tests and unit tests won't
 * find this — only a real browser click against a real store.
 */

import { test, expect } from '@playwright/test';

const APP_URL = 'http://localhost:4444';

test.describe('Tab navigation', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(APP_URL, { waitUntil: 'domcontentloaded', timeout: 15000 });
    test.skip(
      await page.locator('[data-testid="onboarding"]').isVisible().catch(() => false),
      'App in onboarding state',
    );
    // Wait for the tab bar to render
    await page.getByRole('tablist', { name: /content views/i }).waitFor({ timeout: 15000 });
  });

  // Every visible tab must be clickable and change aria-selected state.
  // If this fails for a tab, either TIER_VIEWS has drifted OR the tab is
  // rendered but setActiveView is rejecting it.
  const ALL_NAVIGABLE_TABS = [
    'briefing',
    'preemption',
    'blindspots',
    'chapters',
    'results',
    'playbook',
    'insights',
    'saved',
    'profile',
    'console',
  ];

  for (const tabId of ALL_NAVIGABLE_TABS) {
    test(`clicking tab "${tabId}" changes selection`, async ({ page }) => {
      const tab = page.getByRole('tab', { selected: false }).filter({
        hasText: new RegExp(`nav\\.${tabId.replace('blindspots', 'blindspots')}`, 'i'),
      }).first();

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

    // No React error boundary fallback with "Something went wrong"
    const errorBoundary = page.getByText(/something went wrong/i);
    expect(await errorBoundary.count()).toBe(0);
  });

  test('blindspots tab renders its view without error overlay', async ({ page }) => {
    const blindSpotsTab = page.getByRole('tab').filter({ hasText: /blind ?spots?/i }).first();
    const visible = await blindSpotsTab.isVisible().catch(() => false);
    test.skip(!visible, 'Blind Spots tab not visible at current tier');

    await blindSpotsTab.click();
    await expect(blindSpotsTab).toHaveAttribute('aria-selected', 'true', { timeout: 3000 });

    await page.waitForTimeout(500);

    const errorOverlay = page.locator('vite-error-overlay');
    expect(await errorOverlay.count()).toBe(0);

    const errorBoundary = page.getByText(/something went wrong/i);
    expect(await errorBoundary.count()).toBe(0);
  });
});
