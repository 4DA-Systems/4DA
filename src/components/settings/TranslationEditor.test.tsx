// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { TranslationEditor } from './TranslationEditor';

// Mock Tauri invoke
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe('TranslationEditor', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_translation_status') {
        return Promise.resolve({
          language: 'de',
          total_keys: 100,
          translated_keys: 75,
          percentage: 75.0,
        });
      }
      if (cmd === 'get_all_translations') {
        return Promise.resolve({
          'ui:app.title': { english: '4DA', translated: '4DA', status: 'translated' },
          'ui:app.tagline': { english: 'All signal. No feed.', translated: 'Alles Signal. Kein Feed.', status: 'translated' },
          'coach:coach.title': { english: 'STREETS Coach', translated: null, status: 'untranslated' },
          'errors:error.generic': { english: 'Something went wrong', translated: 'Etwas ist schiefgelaufen', status: 'overridden' },
        });
      }
      return Promise.resolve(null);
    });
  });

  it('renders completeness bar and fetches translation status', async () => {
    render(<TranslationEditor language="de" />);
    // The mock t() returns the key, so check for the completeness key text
    await waitFor(() => {
      expect(screen.getByText('settings.translations.completeness')).toBeInTheDocument();
    });
    // Verify both backend calls were made
    expect(mockInvoke).toHaveBeenCalledWith('get_translation_status', { lang: 'de' });
    expect(mockInvoke).toHaveBeenCalledWith('get_all_translations', { lang: 'de' });
  });

  it('renders the search input for filtering translations', async () => {
    render(<TranslationEditor language="de" />);
    await waitFor(() => {
      expect(screen.getByPlaceholderText('settings.translations.search')).toBeInTheDocument();
    });
  });

  it('filters translation rows by search text', async () => {
    render(<TranslationEditor language="de" />);
    await waitFor(() => {
      expect(screen.getByText('app.title')).toBeInTheDocument();
    });
    const searchInput = screen.getByPlaceholderText('settings.translations.search');
    fireEvent.change(searchInput, { target: { value: 'tagline' } });
    expect(screen.getByText('app.tagline')).toBeInTheDocument();
    expect(screen.queryByText('app.title')).not.toBeInTheDocument();
  });

  it('filters by namespace when a namespace filter button is clicked', async () => {
    render(<TranslationEditor language="de" />);
    await waitFor(() => {
      expect(screen.getByText('app.title')).toBeInTheDocument();
    });
    // Click the "coach" namespace filter button
    const coachFilter = screen.getByRole('button', { name: 'coach' });
    fireEvent.click(coachFilter);
    await waitFor(() => {
      expect(screen.getByText('coach.title')).toBeInTheDocument();
      expect(screen.queryByText('app.title')).not.toBeInTheDocument();
    });
  });

  it('calls save_translation_override when editing and saving a value', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_translation_status') {
        return Promise.resolve({ language: 'de', total_keys: 100, translated_keys: 75, percentage: 75.0 });
      }
      if (cmd === 'get_all_translations') {
        return Promise.resolve({
          'ui:app.title': { english: '4DA', translated: '4DA', status: 'translated' },
        });
      }
      if (cmd === 'save_translation_override') {
        return Promise.resolve(null);
      }
      return Promise.resolve(null);
    });

    render(<TranslationEditor language="de" />);
    await waitFor(() => {
      expect(screen.getByText('app.title')).toBeInTheDocument();
    });

    // Click edit button on the row
    const editButton = screen.getByLabelText('settings.translations.edit');
    fireEvent.click(editButton);

    // Type a new value in the input that appears
    const editInput = screen.getByDisplayValue('4DA');
    fireEvent.change(editInput, { target: { value: '4DA - Neu' } });

    // Click save
    const saveButton = screen.getByLabelText('settings.translations.save');
    fireEvent.click(saveButton);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('save_translation_override', {
        lang: 'de',
        namespace: 'ui',
        key: 'app.title',
        value: '4DA - Neu',
      });
    });
  });
});
