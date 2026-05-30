// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Regression guard for the backend/frontend language desync bug (2026-05).
//
// The original bug: ContentTranslationProvider only pushed *non-English*
// languages to the backend, so when the UI returned to English the backend
// stayed frozen on the last foreign language and rendered every Rust-generated
// string (action labels, relative times, empty states) in that language.
//
// These tests pin the contract: EVERY language change — including 'en' — is
// synced to the backend via `set_language` (which preserves country/currency),
// not the clobbering `set_locale`.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, waitFor } from '@testing-library/react';

const mockCmd = vi.fn();
vi.mock('../lib/commands', () => ({
  cmd: (...args: unknown[]) => mockCmd(...args),
}));

// Controllable i18n language for the mocked react-i18next hook.
let currentLanguage = 'en';
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ i18n: { language: currentLanguage } }),
}));

import { ContentTranslationProvider } from './ContentTranslationProvider';

function renderProvider() {
  return render(
    <ContentTranslationProvider>
      <div>child</div>
    </ContentTranslationProvider>,
  );
}

describe('ContentTranslationProvider — backend language sync', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    currentLanguage = 'en';
    // Default resolution for get_content_translation_settings / others.
    mockCmd.mockResolvedValue({ enabled: false, provider: 'disabled' });
  });

  it('syncs English to the backend (the regression — en must not be skipped)', async () => {
    currentLanguage = 'en';
    renderProvider();
    await waitFor(() => {
      expect(mockCmd).toHaveBeenCalledWith('set_language', { language: 'en' });
    });
  });

  it('syncs a non-English language to the backend', async () => {
    currentLanguage = 'zh';
    renderProvider();
    await waitFor(() => {
      expect(mockCmd).toHaveBeenCalledWith('set_language', { language: 'zh' });
    });
  });

  it('never uses set_locale (which would clobber country/currency)', async () => {
    currentLanguage = 'zh';
    renderProvider();
    await waitFor(() => {
      expect(mockCmd).toHaveBeenCalledWith('set_language', { language: 'zh' });
    });
    const usedSetLocale = mockCmd.mock.calls.some((c) => c[0] === 'set_locale');
    expect(usedSetLocale).toBe(false);
  });

  it('re-syncs when the language changes back to English', async () => {
    currentLanguage = 'zh';
    const { rerender } = renderProvider();
    await waitFor(() => {
      expect(mockCmd).toHaveBeenCalledWith('set_language', { language: 'zh' });
    });

    currentLanguage = 'en';
    rerender(
      <ContentTranslationProvider>
        <div>child</div>
      </ContentTranslationProvider>,
    );
    await waitFor(() => {
      expect(mockCmd).toHaveBeenCalledWith('set_language', { language: 'en' });
    });
  });
});
