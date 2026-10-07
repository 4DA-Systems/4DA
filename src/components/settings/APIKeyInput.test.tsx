// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { APIKeyInput } from './APIKeyInput';
import { IDLE_VALIDATION } from './ai-provider-constants';
import type { Settings } from '../../types';
import type { SettingsForm } from './ai-provider-types';

// Audit 2026-10-07: the saved-key line must follow the backend's storage
// posture — never claim secure storage when the key is kept in settings.json.
function settingsWith(posture: 'keychain' | 'file_fallback' | undefined): Settings {
  return {
    llm: { provider: 'anthropic', model: 'm', has_api_key: true, base_url: null },
    ...(posture
      ? { secret_storage: { mode: posture, secrets: { llm_api_key: posture } } }
      : {}),
  } as unknown as Settings;
}

const form = { provider: 'anthropic', apiKey: '', baseUrl: '' } as unknown as SettingsForm;

function renderWith(posture: 'keychain' | 'file_fallback' | undefined) {
  render(
    <APIKeyInput
      settings={settingsWith(posture)}
      settingsForm={form}
      setSettingsForm={vi.fn()}
      validation={IDLE_VALIDATION}
      validateKey={vi.fn()}
    />,
  );
}

describe('APIKeyInput — truthful storage copy', () => {
  it('says secure storage when the key is in the OS credential store', () => {
    renderWith('keychain');
    expect(screen.getByText(/settings\.ai\.keySavedSecure/)).toBeInTheDocument();
    expect(screen.queryByText(/settings\.ai\.keySavedFile/)).toBeNull();
  });

  it('says settings.json when the credential store could not hold it', () => {
    renderWith('file_fallback');
    expect(screen.getByText(/settings\.ai\.keySavedFile/)).toBeInTheDocument();
    expect(screen.queryByText(/settings\.ai\.keySavedSecure/)).toBeNull();
  });
});
