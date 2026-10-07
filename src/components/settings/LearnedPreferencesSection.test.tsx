// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({ cmd: (...args: unknown[]) => cmdMock(...args) }));
vi.mock('../../store', () => ({
  useAppStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ setSettingsStatus: vi.fn() }),
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import { LearnedPreferencesSection } from './LearnedPreferencesSection';
import { facetLabel } from './learned-facet-label';

function facet(id: string, cls: string, key: string, value: string) {
  return {
    facet_id: id,
    class: cls,
    key,
    value,
    stability: 1,
    state: 'active',
    user_state: 'auto',
    evidence_count: 4,
    first_seen_at: 0,
    last_seen_at: 0,
  };
}

describe('LearnedPreferencesSection chips', () => {
  beforeEach(() => cmdMock.mockReset());

  it('labels each chip with the learned key, value as a qualifier', async () => {
    cmdMock.mockResolvedValue({
      facets: [
        facet('interest:rust', 'interest', 'rust', 'engaged'),
        facet('interest:tauri', 'interest', 'tauri', 'engaged'),
        facet('workflow:code_review', 'workflow', 'code_review', 'producing'),
      ],
    });
    render(<LearnedPreferencesSection />);

    const rust = await screen.findByRole('button', { name: /rust/ });
    expect(rust).toHaveTextContent('rust');
    expect(rust).toHaveTextContent('engaged');
    expect(screen.getByRole('button', { name: /tauri/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /code review/ })).toHaveTextContent('producing');
    // No chip is named only by its value.
    expect(screen.queryByRole('button', { name: /^engaged$/ })).toBeNull();
    expect(screen.queryByRole('button', { name: /^producing$/ })).toBeNull();
  });
});

describe('facetLabel', () => {
  it('uses the key as the name and the value as qualifier', () => {
    expect(facetLabel({ class: 'interest', key: 'rust', value: 'engaged' })).toEqual({
      name: 'rust',
      qualifier: 'engaged',
    });
  });

  it('humanizes underscores', () => {
    expect(facetLabel({ class: 'topic_affinity', key: 'machine_learning', value: 'saved' }).name).toBe(
      'machine learning',
    );
  });

  it('drops a qualifier identical to the name and falls back to value when key is empty', () => {
    expect(facetLabel({ class: 'interest', key: 'high', value: 'high' }).qualifier).toBe('');
    expect(facetLabel({ class: 'interest', key: '', value: 'engaged' })).toEqual({
      name: 'engaged',
      qualifier: '',
    });
  });
});
