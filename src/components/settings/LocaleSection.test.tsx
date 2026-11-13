// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { LocaleSection } from './LocaleSection';
import { aiRunsOnMachine } from './locale-constants';
import { useAppStore } from '../../store';

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

// Audit 2026-11-11: a non-English UI with a cloud AI key sent card text to
// that model for translation, and the consent toggle only appeared for
// DeepL / Google / Azure. The toggle now covers the AI model too.
describe('LocaleSection — cloud translation consent', () => {
  const txConfig = { provider: 'llm', api_key: '', auto_translate: true, translate_descriptions: false, cloud_translation_consent: false };

  function renderWith(llm: { provider: string; base_url: string | null }) {
    cmdMock.mockImplementation((name: string): Promise<unknown> => {
      if (name === 'get_locale') return Promise.resolve({ country: 'DE', language: 'de', currency: 'EUR' });
      if (name === 'get_translation_config') return Promise.resolve(txConfig);
      return Promise.reject(new Error('not mocked'));
    });
    useAppStore.setState({ settings: { llm: { ...llm, model: '', has_api_key: true } } as never });
    render(<LocaleSection />);
  }

  it('asks for consent when the AI model translating is a cloud model', async () => {
    renderWith({ provider: 'anthropic', base_url: null });
    expect(await screen.findByText('settings.translation.cloudConsentLlmHelp')).toBeInTheDocument();
  });

  it('does not ask when the model runs on this machine', async () => {
    renderWith({ provider: 'ollama', base_url: null });
    await screen.findByText('settings.translation.title');
    expect(screen.queryByText('settings.translation.cloudConsentLlmHelp')).not.toBeInTheDocument();
  });
});

describe('aiRunsOnMachine', () => {
  it('reads a loopback base URL as local and anything else as cloud', () => {
    expect(aiRunsOnMachine({ provider: 'openai', base_url: 'http://127.0.0.1:1234/v1' })).toBe(true);
    expect(aiRunsOnMachine({ provider: 'openai', base_url: 'https://api.openai.com/v1' })).toBe(false);
    expect(aiRunsOnMachine({ provider: 'anthropic', base_url: null })).toBe(false);
    expect(aiRunsOnMachine({ provider: 'ollama', base_url: null })).toBe(true);
  });
});
