import { test, expect } from '@playwright/test';

import { installTauriIpcMock, ipcLog, mockSettings } from './support/app';

/**
 * Cloud-LLM disclosure + IPC param casing.
 *
 * Consent is recorded by the BACKEND when a cloud provider is saved with a key
 * (settings/manager.rs `set_llm_provider`: "BYOK = informed consent, recorded
 * HERE at configuration time"). The frontend's job is (1) show the disclosure
 * of what gets sent before the user saves, and (2) send `set_llm_provider`
 * with camelCase params — Tauri maps camelCase to the Rust snake_case args, so
 * a snake_case key is silently dropped (the original bug this spec guarded).
 * The earlier version asserted a frontend `set_privacy_config` call that the
 * consent move made obsolete.
 */
test.describe('Privacy Disclosure Fix', () => {
  test('cloud provider save shows the disclosure and sends camelCase IPC params', async ({ page }) => {
    await installTauriIpcMock(page, {
      settings: mockSettings({ provider: 'anthropic', model: 'claude-sonnet-5', has_api_key: false }),
      responses: {
        validate_api_key: { valid: true, format_ok: true, error: null, model_access: [] },
      },
    });
    await page.goto('/');

    await page.locator('[data-settings-trigger]').click();
    const dialog = page.getByRole('dialog', { name: /settings/i });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('tab', { name: 'Intelligence' }).click();

    const provider = dialog.locator('#ai-provider-select');
    await expect(provider).toHaveValue('anthropic');
    // Informed consent: the disclosure is on screen before anything is saved.
    await expect(dialog.getByText(/what gets sent: when you use a cloud model/i)).toBeVisible();

    // Fake, scanner-safe placeholder (no provider prefix); validation is mocked.
    const key = 'e2e-placeholder-key-0123456789';
    await dialog.locator('#ai-api-key').fill(key);
    await dialog.getByRole('button', { name: 'Save AI Configuration' }).click();
    await expect(dialog.getByText('AI configuration saved successfully.')).toBeVisible();

    const calls = await ipcLog(page);
    const providerCalls = calls.filter((c) => c.cmd === 'set_llm_provider');
    expect(providerCalls).toHaveLength(1);
    expect(providerCalls[0]?.args).toMatchObject({ provider: 'anthropic', apiKey: key, model: 'claude-sonnet-5' });

    // Every set_* command must use camelCase keys.
    const setCalls = calls.filter((c) => c.cmd.startsWith('set_'));
    expect(setCalls.length).toBeGreaterThan(0);
    for (const call of setCalls) {
      expect(Object.keys(call.args).filter((k) => k.includes('_')), `${call.cmd} args`).toEqual([]);
    }
  });
});
