import { test, expect } from '@playwright/test';

import { MAIN_TABS, appBar, gotoMainShell, mainTablist } from './support/app';

/**
 * The analysis controls live in the app bar (UnifiedAppBar, a banner
 * landmark); the old standalone action bar — a role="toolbar" with an overflow
 * menu — no longer exists, so the overflow-menu test was removed rather than
 * left as a permanent skip.
 */
test.describe('Analysis Flow & Action Bar', () => {
  test.beforeEach(async ({ page }) => {
    await gotoMainShell(page);
  });

  test('app bar is a labelled banner landmark', async ({ page }) => {
    await expect(appBar(page)).toBeVisible();
  });

  test('analyze button is present and clickable', async ({ page }) => {
    const analyze = appBar(page).getByRole('button', { name: 'Run analysis' });
    await expect(analyze).toBeVisible();
    await expect(analyze).toBeEnabled();
  });

  test('status area has aria-live for screen readers', async ({ page }) => {
    const status = appBar(page).getByRole('status');
    await expect(status).toHaveAttribute('aria-live', 'polite');
    await expect(status).toHaveText('Ready to analyze');
  });

  test('search input is functional', async ({ page }) => {
    const search = appBar(page).getByRole('combobox', { name: 'Search 4DA' });
    await search.fill('test query');
    await expect(search).toHaveValue('test query');
    await search.fill('');
    await expect(search).toHaveValue('');
  });

  test('tabs have correct selection state', async ({ page }) => {
    const tabs = mainTablist(page).getByRole('tab');
    await expect(tabs).toHaveCount(MAIN_TABS.length);
    await expect(mainTablist(page).getByRole('tab', { selected: true })).toHaveCount(1);
  });

  test('selected tab has matching aria-controls panel', async ({ page }) => {
    const selected = mainTablist(page).getByRole('tab', { selected: true });
    const controls = await selected.getAttribute('aria-controls');
    expect(controls).toBeTruthy();
    await expect(page.locator(`#${controls}`)).toBeVisible();
  });
});
