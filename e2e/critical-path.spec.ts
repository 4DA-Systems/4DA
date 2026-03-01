import { test, expect, type Page } from '@playwright/test';

/**
 * 4DA Critical Path E2E Tests
 *
 * These tests verify the frontend renders and navigates correctly when
 * the Vite dev server is running on localhost:4444.
 *
 * The Tauri backend is NOT required — invoke() calls will fail, but the
 * React app still mounts. The app may show:
 *   1. A splash screen (briefly), then
 *   2. Onboarding (first-run, no persisted data), or
 *   3. The main briefing/results view
 *
 * Tests handle both states by waiting for the app shell to stabilize.
 */

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Wait for the app to move past the splash screen and reach a stable state.
 * Returns which state the app landed in: 'onboarding' or 'main'.
 */
async function waitForAppReady(page: Page): Promise<'onboarding' | 'main'> {
  // The app always starts with the splash screen. Wait for it to disappear
  // and for either the onboarding dialog or the main app shell to appear.
  // Use a generous timeout since the splash has a minimum display time.

  // Wait for either: the onboarding dialog, the main tablist, or the action bar
  const onboarding = page.getByRole('dialog', { name: /setup wizard/i });
  const tablist = page.getByRole('tablist', { name: /content views/i });

  // Wait up to 15s for either element to be visible
  const result = await Promise.race([
    onboarding.waitFor({ state: 'visible', timeout: 15_000 }).then(() => 'onboarding' as const),
    tablist.waitFor({ state: 'visible', timeout: 15_000 }).then(() => 'main' as const),
  ]);

  return result;
}

/**
 * If the app is showing onboarding, skip it to reach the main view.
 * This is a best-effort — onboarding completion calls invoke() which
 * will fail without the Tauri backend. We may need to just verify
 * onboarding rendered correctly instead.
 */
async function getToMainView(page: Page): Promise<'main' | 'onboarding'> {
  const appState = await waitForAppReady(page);
  if (appState === 'main') return 'main';

  // Onboarding is showing. Without the Tauri backend we cannot complete it,
  // so tests that need the main view will be skipped when onboarding is present.
  return 'onboarding';
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

test.describe('4DA Critical Path', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test('app loads without crash', async ({ page }) => {
    // The page should not be blank — verify that some content renders.
    // The splash screen shows "4DA" as an h1, and so does the main app.
    // We just need to confirm the React app mounted at all.
    const body = page.locator('body');
    await expect(body).not.toBeEmpty();

    // Wait for the app to move past initial loading — either the splash
    // screen text or the app shell should be visible within a few seconds.
    const appContent = page.locator('text=4DA');
    await expect(appContent.first()).toBeVisible({ timeout: 10_000 });
  });

  test('core layout present — onboarding or main shell', async ({ page }) => {
    const appState = await waitForAppReady(page);

    if (appState === 'onboarding') {
      // Verify the onboarding dialog rendered with its setup wizard label
      const dialog = page.getByRole('dialog', { name: /setup wizard/i });
      await expect(dialog).toBeVisible();

      // The onboarding should have step progress indicators
      const stepGroup = page.getByRole('group', { name: /step/i });
      await expect(stepGroup).toBeVisible();
    } else {
      // Main view: verify the tab bar and action bar region are present
      const tablist = page.getByRole('tablist', { name: /content views/i });
      await expect(tablist).toBeVisible();

      const actionBar = page.getByRole('region', { name: /analysis controls/i });
      await expect(actionBar).toBeVisible();

      // Verify the header with app title is visible
      const heading = page.getByRole('heading', { name: '4DA', level: 1 });
      await expect(heading).toBeVisible();
    }
  });

  test('navigation works — view tabs switch content', async ({ page }) => {
    const appState = await getToMainView(page);
    test.skip(appState === 'onboarding', 'Cannot test navigation during onboarding (no backend to skip it)');

    const tablist = page.getByRole('tablist', { name: /content views/i });
    await expect(tablist).toBeVisible();

    // Define the tabs we expect to find
    const tabNames = ['Intelligence', 'All Results', 'Insights', 'Saved', 'Toolkit', 'Playbook'];

    // Verify all tabs exist
    for (const name of tabNames) {
      const tab = tablist.getByRole('tab', { name });
      await expect(tab).toBeVisible();
    }

    // Click through each tab and verify it becomes selected
    for (const name of tabNames) {
      const tab = tablist.getByRole('tab', { name });
      await tab.click();
      await expect(tab).toHaveAttribute('aria-selected', 'true');

      // Verify the corresponding tab panel renders (by checking
      // that the previously selected tab is no longer selected,
      // except for the current one)
      for (const otherName of tabNames) {
        if (otherName === name) continue;
        const otherTab = tablist.getByRole('tab', { name: otherName });
        await expect(otherTab).toHaveAttribute('aria-selected', 'false');
      }
    }
  });

  test('settings modal opens and closes', async ({ page }) => {
    const appState = await getToMainView(page);
    test.skip(appState === 'onboarding', 'Cannot test settings during onboarding (no backend to skip it)');

    // Open settings via the Settings button in the header
    const settingsButton = page.getByRole('button', { name: /settings/i });
    await expect(settingsButton).toBeVisible();
    await settingsButton.click();

    // Verify the settings dialog appears
    const dialog = page.getByRole('dialog', { name: /settings/i });
    await expect(dialog).toBeVisible();

    // Verify the settings modal has its title
    const title = page.locator('#settings-modal-title');
    await expect(title).toHaveText('Settings');

    // Verify settings tabs are present
    const settingsTablist = dialog.getByRole('tablist');
    await expect(settingsTablist).toBeVisible();

    const settingsTabs = ['General', 'Sources', 'Profile', 'Discovery', 'Health'];
    for (const tabName of settingsTabs) {
      await expect(settingsTablist.getByRole('tab', { name: tabName })).toBeVisible();
    }

    // Close the settings modal via the close button
    const closeButton = dialog.getByRole('button', { name: /close settings/i });
    await closeButton.click();

    // Verify the dialog is gone
    await expect(dialog).not.toBeVisible();
  });

  test('settings modal opens via comma keyboard shortcut', async ({ page }) => {
    const appState = await getToMainView(page);
    test.skip(appState === 'onboarding', 'Cannot test keyboard shortcuts during onboarding');

    // Press comma to open settings
    await page.keyboard.press(',');

    const dialog = page.getByRole('dialog', { name: /settings/i });
    await expect(dialog).toBeVisible();

    // Close with Escape
    await page.keyboard.press('Escape');
    await expect(dialog).not.toBeVisible();
  });

  test('keyboard shortcuts modal opens and closes', async ({ page }) => {
    const appState = await getToMainView(page);
    test.skip(appState === 'onboarding', 'Cannot test keyboard shortcuts during onboarding');

    // Press '?' to open the keyboard shortcuts modal
    await page.keyboard.press('?');

    // The keyboard shortcuts modal should appear
    const dialog = page.getByRole('dialog', { name: /keyboard shortcuts/i });
    await expect(dialog).toBeVisible();

    // Verify it lists some expected shortcuts
    await expect(dialog.locator('text=Run analysis')).toBeVisible();
    await expect(dialog.locator('text=Open settings')).toBeVisible();
    await expect(dialog.locator('text=Show this help')).toBeVisible();

    // Close via the close button
    const closeButton = dialog.getByRole('button', { name: /close keyboard shortcuts/i });
    await closeButton.click();

    await expect(dialog).not.toBeVisible();
  });

  test('app recovers from rapid navigation', async ({ page }) => {
    const appState = await getToMainView(page);
    test.skip(appState === 'onboarding', 'Cannot test navigation during onboarding (no backend to skip it)');

    const tablist = page.getByRole('tablist', { name: /content views/i });
    const tabs = tablist.getByRole('tab');
    const count = await tabs.count();

    // Rapid-fire tab switching — stress test lazy loading
    for (let round = 0; round < 2; round++) {
      for (let i = 0; i < count; i++) {
        await tabs.nth(i).click();
        await page.waitForTimeout(100); // deliberately fast
      }
    }

    // App should still be functional after rapid switching
    await tabs.first().click();
    await expect(tabs.first()).toHaveAttribute('aria-selected', 'true');
  });

  test('keyboard shortcuts modal closes with Escape', async ({ page }) => {
    const appState = await getToMainView(page);
    test.skip(appState === 'onboarding', 'Cannot test keyboard shortcuts during onboarding');

    // Open keyboard shortcuts
    await page.keyboard.press('?');

    const dialog = page.getByRole('dialog', { name: /keyboard shortcuts/i });
    await expect(dialog).toBeVisible();

    // Close with Escape
    await page.keyboard.press('Escape');
    await expect(dialog).not.toBeVisible();
  });
});
