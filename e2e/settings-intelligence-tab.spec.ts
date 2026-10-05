import { test, expect } from '@playwright/test';

import { crashFallbacks, installTauriIpcMock, mockSettings } from './support/app';

/**
 * Settings > Intelligence for a local (Ollama) user: Anthropic is the first,
 * recommended provider; the BYOK nudge offers a one-click switch to Anthropic,
 * which reveals the API-key field and hides the nudge.
 */
test.describe('Settings Intelligence Tab', () => {
  test('validate Intelligence tab UI', async ({ page }) => {
    await installTauriIpcMock(page, {
      settings: mockSettings({ provider: 'ollama', model: 'llama3.2', base_url: 'http://localhost:11434' }),
    });
    await page.goto('/');

    await page.locator('[data-settings-trigger]').click();
    const dialog = page.getByRole('dialog', { name: /settings/i });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('tab', { name: 'Intelligence' }).click();

    const provider = dialog.locator('#ai-provider-select');
    await expect(provider).toHaveValue('ollama');
    const firstOption = provider.locator('option').first();
    await expect(firstOption).toHaveAttribute('value', 'anthropic');
    await expect(firstOption).toContainText('Anthropic');
    await expect(firstOption).toContainText('Recommended');

    // Local provider: the nudge is shown and there is no API-key field.
    const nudge = dialog.getByText('Get better results with a cloud API key');
    await expect(nudge).toBeVisible();
    await expect(dialog.locator('#ai-api-key')).toHaveCount(0);

    await dialog.getByRole('button', { name: 'Switch to Anthropic' }).click();
    await expect(provider).toHaveValue('anthropic');
    await expect(dialog.locator('#ai-api-key')).toBeVisible();
    await expect(nudge).toBeHidden();

    // Selecting Ollama again brings the nudge back.
    await provider.selectOption('ollama');
    await expect(nudge).toBeVisible();

    await expect(crashFallbacks(page)).toHaveCount(0);
  });
});
