// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { LocaleSection } from './LocaleSection';

const cmdMock = vi.fn((name: string): Promise<unknown> => {
  if (name === 'get_locale') return Promise.resolve({ country: 'AU', language: 'en', currency: 'AUD' });
  return Promise.reject(new Error('not mocked'));
});
vi.mock('../../lib/commands', () => ({ cmd: (name: string) => cmdMock(name) }));

describe('LocaleSection — programmatic labels (audit L9)', () => {
  it('names the country, language and currency selects by their visible labels', async () => {
    render(<LocaleSection />);
    // The passthrough i18n mock renders each label as its key.
    const country = await screen.findByLabelText('settings.locale.country');
    const language = screen.getByLabelText('settings.locale.language');
    const currency = screen.getByLabelText('settings.locale.currency');
    expect(country.tagName).toBe('SELECT');
    expect(language.tagName).toBe('SELECT');
    expect(currency.tagName).toBe('SELECT');
    expect(screen.getByRole('combobox', { name: 'settings.locale.currency' })).toHaveValue('AUD');
    // Every select on the section has an accessible name.
    for (const select of screen.getAllByRole('combobox')) {
      expect((select as HTMLSelectElement).labels?.length ?? 0).toBeGreaterThan(0);
    }
  });
});
