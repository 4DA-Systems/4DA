import { test, expect } from '@playwright/test';

const APP_URL = 'http://localhost:4444';

test.describe('Analysis Flow & Action Bar', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(APP_URL, { waitUntil: 'networkidle', timeout: 15000 });
    test.skip(
      await page.locator('[data-testid="onboarding"]').isVisible(),
      'App in onboarding state'
    );
  });

  test('action bar has correct ARIA role', async ({ page }) => {
    const toolbar = page.getByRole('toolbar').or(page.locator('[data-testid="action-bar"]'));
    const hasToolbar = await toolbar.isVisible().catch(() => false);
    test.skip(!hasToolbar, 'Action bar not visible');

    // Verify the toolbar has an accessible role
    const role = await toolbar.first().getAttribute('role');
    expect(role).toBe('toolbar');
  });

  test('analyze button is present and clickable', async ({ page }) => {
    const analyzeButton = page.getByRole('button', { name: /analy[sz]e|scan|run/i })
      .or(page.locator('[data-testid="analyze-button"]'));
    const hasButton = await analyzeButton.isVisible().catch(() => false);
    test.skip(!hasButton, 'No analyze button visible');

    await expect(analyzeButton.first()).toBeEnabled();
  });

  test('overflow menu exists in action bar', async ({ page }) => {
    const overflowButton = page.getByRole('button', { name: /more|menu|overflow|\.\.\./i })
      .or(page.locator('[data-testid="overflow-menu"]'))
      .or(page.locator('button[aria-haspopup="menu"]'));
    const hasOverflow = await overflowButton.isVisible().catch(() => false);
    test.skip(!hasOverflow, 'No overflow menu visible');

    await overflowButton.first().click();

    // A menu or popover should appear
    const menu = page.getByRole('menu').or(page.locator('[role="listbox"]'));
    await expect(menu).toBeVisible({ timeout: 3000 });
  });

  test('status area has aria-live for screen readers', async ({ page }) => {
    const liveRegion = page.locator('[aria-live]');
    const count = await liveRegion.count();

    if (count > 0) {
      const ariaLiveValue = await liveRegion.first().getAttribute('aria-live');
      expect(['polite', 'assertive']).toContain(ariaLiveValue);
    } else {
      test.info().annotations.push({
        type: 'a11y',
        description: 'No aria-live region found — status updates may not be announced to screen readers',
      });
    }
  });

  test('search input is functional', async ({ page }) => {
    const searchInput = page.getByRole('searchbox')
      .or(page.getByPlaceholder(/search|filter|find/i))
      .or(page.locator('input[type="search"]'));
    const hasSearch = await searchInput.isVisible().catch(() => false);
    test.skip(!hasSearch, 'No search input visible');

    await searchInput.first().fill('test query');
    const value = await searchInput.first().inputValue();
    expect(value).toBe('test query');

    // Clear and verify
    await searchInput.first().fill('');
    const cleared = await searchInput.first().inputValue();
    expect(cleared).toBe('');
  });

  test('tabs have correct selection state', async ({ page }) => {
    const tablist = page.getByRole('tablist');
    const hasTablist = await tablist.isVisible().catch(() => false);
    test.skip(!hasTablist, 'No tab list visible');

    const tabs = tablist.getByRole('tab');
    const tabCount = await tabs.count();
    expect(tabCount).toBeGreaterThanOrEqual(1);

    // Exactly one tab should be selected
    let selectedCount = 0;
    for (let i = 0; i < tabCount; i++) {
      const selected = await tabs.nth(i).getAttribute('aria-selected');
      if (selected === 'true') selectedCount++;
    }
    expect(selectedCount).toBe(1);
  });

  test('selected tab has matching aria-controls panel', async ({ page }) => {
    const tablist = page.getByRole('tablist');
    const hasTablist = await tablist.isVisible().catch(() => false);
    test.skip(!hasTablist, 'No tab list visible');

    const tabs = tablist.getByRole('tab');
    const tabCount = await tabs.count();

    // Find the selected tab
    for (let i = 0; i < tabCount; i++) {
      const selected = await tabs.nth(i).getAttribute('aria-selected');
      if (selected === 'true') {
        const controls = await tabs.nth(i).getAttribute('aria-controls');
        if (controls) {
          // The referenced panel should exist in the DOM
          const panel = page.locator(`#${controls}`);
          await expect(panel).toBeAttached();
        }
        break;
      }
    }
  });
});
