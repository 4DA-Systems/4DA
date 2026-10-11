// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';

import { CommandSearch } from './CommandSearch';
import { useAppStore } from '../../store';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string | Record<string, unknown>) =>
      typeof fallback === 'string' ? fallback : key,
  }),
}));
vi.mock('./search-warm', () => ({ warmSearchBackend: () => false }));
vi.mock('../../lib/frecency', () => ({ frecencyBoost: () => 0, recordPick: () => {} }));

// The Preemption feed resolves only when the test says so, to stage "the user
// typed before the feed arrived".
let releaseFeed: (() => void) | null = null;
const SHARP_FEED = {
  items: [
    {
      id: 'upgrade-plan:npm:sharp',
      kind: 'alert',
      title: 'Upgrade sharp to >= 0.35.5 — clears 1 advisory across 1 project',
      explanation: '',
      confidence: { value: 0.9, provenance: 'heuristic' },
      urgency: 'high',
      evidence: [],
      affected_projects: ['d:/work/navcal'],
      affected_deps: ['sharp'],
      suggested_actions: [],
      lens_hints: { preemption: true },
      created_at: 0,
    },
  ],
  total: 1,
  critical_count: 0,
  high_count: 1,
  score: null,
};

vi.mock('../../lib/commands', () => ({
  cmd: (name: string) => {
    if (name === 'get_preemption_alerts') {
      return new Promise(resolve => {
        releaseFeed = () => resolve(SHARP_FEED);
      });
    }
    if (name === 'natural_language_query') {
      return Promise.resolve({ items: [], ghost_preview: null, is_pro: true, total_count: 0, semantic_pending: false });
    }
    return Promise.resolve(null);
  },
}));

describe('CommandSearch — Your dependencies', () => {
  beforeEach(() => {
    releaseFeed = null;
    useAppStore.setState({ preemptionFeed: null, preemptionLoading: false, preemptionPaywalled: false });
  });

  it('shows the group when the feed lands after the query was typed, with no further keystroke', async () => {
    render(<CommandSearch onAnalyze={() => {}} onOpenSettings={() => {}} />);
    const input = screen.getByRole('combobox');
    fireEvent.focus(input); // opening the field starts the feed load
    fireEvent.change(input, { target: { value: 'sharp vulnerability' } });
    await waitFor(() => expect(releaseFeed).not.toBeNull());
    expect(screen.queryByText('Your dependencies')).toBeNull();

    await act(async () => {
      releaseFeed?.();
    });

    await waitFor(() => expect(screen.getByText('Your dependencies')).toBeTruthy());
    expect(screen.getByText(/Upgrade sharp to >= 0\.35\.5/)).toBeTruthy();
  });

  it('a feed landing with an empty field adds no rows', async () => {
    render(<CommandSearch onAnalyze={() => {}} onOpenSettings={() => {}} />);
    fireEvent.focus(screen.getByRole('combobox'));
    await waitFor(() => expect(releaseFeed).not.toBeNull());
    await act(async () => {
      releaseFeed?.();
    });
    expect(screen.queryByText('Your dependencies')).toBeNull();
  });
});
