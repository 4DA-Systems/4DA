// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import PreemptionView from './PreemptionView';

// The "Not affected" action parks an advisory in a persistent, reviewable
// bucket instead of a black hole: the user can always find what they dismissed,
// restore it if they were wrong, or delete it permanently. These tests pin that
// contract via localStorage state (i18n-independent) so they hold regardless of
// how the test i18n resolves label keys.

vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => false }));
vi.mock('../SignalUpgradeCTA', () => ({ SignalUpgradeCTA: () => <div /> }));
vi.mock('./PreemptionTierSection', () => ({ PreemptionTierSection: () => <div /> }));

let mockState: Record<string, unknown> = {};
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

function setState(overrides: Record<string, unknown> = {}) {
  mockState = {
    preemptionFeed: null,
    preemptionLoading: false,
    preemptionError: null,
    preemptionPaywalled: false,
    loadPreemption: vi.fn(),
    ...overrides,
  };
}

const NA_KEY = 'preemption_not_affected';
const DEL_KEY = 'preemption_deleted';

function seed() {
  localStorage.setItem(
    NA_KEY,
    JSON.stringify([{ id: 'osv-1', ts: 1, title: 'CVE in axios', deps: ['axios'] }]),
  );
}

describe('PreemptionView — Not affected bucket', () => {
  beforeEach(() => {
    localStorage.clear();
    setState();
  });

  it('renders a collapsed review bucket when there are stored not-affected items', () => {
    seed();
    render(<PreemptionView />);
    // The only aria-expanded toggle in this state is the Not-affected header.
    expect(screen.getByRole('button', { expanded: false })).toBeInTheDocument();
    // Collapsed: the entry itself is not yet in the DOM.
    expect(screen.queryByText('CVE in axios')).toBeNull();
  });

  it('shows no bucket at all when nothing was marked not affected', () => {
    render(<PreemptionView />);
    expect(screen.queryByRole('button', { expanded: false })).toBeNull();
  });

  it('Restore removes the item from the bucket so it can resurface', () => {
    seed();
    render(<PreemptionView />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));
    const row = screen.getByText('CVE in axios').closest('li') as HTMLElement;
    fireEvent.click(within(row).getAllByRole('button')[0]); // [0] = Restore, [1] = Delete
    expect(JSON.parse(localStorage.getItem(NA_KEY) || '[]')).toHaveLength(0);
    expect(JSON.parse(localStorage.getItem(DEL_KEY) || '[]')).toHaveLength(0);
  });

  it('Delete permanently clears the bucket AND records the id as suppressed', () => {
    seed();
    render(<PreemptionView />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));
    const row = screen.getByText('CVE in axios').closest('li') as HTMLElement;
    fireEvent.click(within(row).getAllByRole('button')[1]); // Delete permanently
    expect(JSON.parse(localStorage.getItem(NA_KEY) || '[]')).toHaveLength(0);
    expect(JSON.parse(localStorage.getItem(DEL_KEY) || '[]')).toContain('osv-1');
  });
});
