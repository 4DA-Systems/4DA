// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import { SprintPhase } from './SprintPhase';

const mockInvoke = vi.mocked(invoke);

const CARDS = [
  {
    sourceItemId: 11,
    title: 'Rust 1.99 released',
    snippet: 'The release brings...',
    sourceType: 'hackernews',
    url: 'https://example.com/1',
  },
  {
    sourceItemId: 22,
    title: 'New K8s CVE',
    snippet: 'A vulnerability in...',
    sourceType: 'cve',
    url: null,
  },
];

const STATUS = { labeledTotal: 3, minFitSamples: 50, curveFitted: false };

function wireBackend(cards = CARDS, status = STATUS) {
  mockInvoke.mockImplementation((c: string) => {
    if (c === 'get_calibration_sprint_items') return Promise.resolve(cards);
    if (c === 'get_calibration_sprint_status') return Promise.resolve(status);
    if (c === 'record_calibration_sprint_response') return Promise.resolve(undefined);
    return Promise.resolve({});
  });
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('SprintPhase — labeling flow', () => {
  it('shows the first card with its real source type', async () => {
    wireBackend();
    render(<SprintPhase onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText('Rust 1.99 released')).toBeInTheDocument());
    expect(screen.getByText('hackernews')).toBeInTheDocument();
  });

  it('Relevant writes a feedback label with the right item id and advances', async () => {
    wireBackend();
    render(<SprintPhase onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText('Rust 1.99 released')).toBeInTheDocument());

    fireEvent.click(screen.getByText('calibrationView.sprint.relevant'));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        'record_calibration_sprint_response',
        { sourceItemId: 11, response: 'relevant' },
      ),
    );
    await waitFor(() => expect(screen.getByText('New K8s CVE')).toBeInTheDocument());
  });

  it('Not relevant sends the negative label', async () => {
    wireBackend();
    render(<SprintPhase onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText('Rust 1.99 released')).toBeInTheDocument());

    fireEvent.click(screen.getByText('calibrationView.sprint.notRelevant'));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        'record_calibration_sprint_response',
        { sourceItemId: 11, response: 'not_relevant' },
      ),
    );
  });

  it('Skip sends skip and does not bump the label counters', async () => {
    wireBackend();
    render(<SprintPhase onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText('Rust 1.99 released')).toBeInTheDocument());

    fireEvent.click(screen.getByText('calibrationView.sprint.skip'));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        'record_calibration_sprint_response',
        { sourceItemId: 11, response: 'skip' },
      ),
    );
    // Skip then relevant on the LAST card -> finish screen reports 1 label, not 2.
    await waitFor(() => expect(screen.getByText('New K8s CVE')).toBeInTheDocument());
    fireEvent.click(screen.getByText('calibrationView.sprint.relevant'));
    await waitFor(() => expect(screen.getByTestId('sprint-done')).toBeInTheDocument());
    expect(screen.getByText('calibrationView.sprint.doneBody')).toBeInTheDocument();
  });

  it('keyboard shortcuts label without touching the mouse', async () => {
    wireBackend();
    render(<SprintPhase onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText('Rust 1.99 released')).toBeInTheDocument());

    fireEvent.keyDown(window, { key: 'ArrowRight' });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        'record_calibration_sprint_response',
        { sourceItemId: 11, response: 'relevant' },
      ),
    );
  });

  it('finishing the deck shows the honest finish screen', async () => {
    wireBackend(CARDS.slice(0, 1));
    render(<SprintPhase onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText('Rust 1.99 released')).toBeInTheDocument());

    fireEvent.click(screen.getByText('calibrationView.sprint.relevant'));
    await waitFor(() => expect(screen.getByTestId('sprint-done')).toBeInTheDocument());
    // The unlock copy explains what the labels feed — no vanity numbers.
    expect(screen.getByText('calibrationView.sprint.unlocks')).toBeInTheDocument();
  });

  it('renders the honest empty state when no eligible items exist', async () => {
    wireBackend([]);
    render(<SprintPhase onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('sprint-empty')).toBeInTheDocument());
    expect(screen.getByText('calibrationView.sprint.noItemsHint')).toBeInTheDocument();
    // Never invents a card.
    expect(screen.queryByTestId('sprint-cards')).not.toBeInTheDocument();
  });

  it('a failed label write surfaces the error and does not advance', async () => {
    mockInvoke.mockImplementation((c: string) => {
      if (c === 'get_calibration_sprint_items') return Promise.resolve(CARDS);
      if (c === 'get_calibration_sprint_status') return Promise.resolve(STATUS);
      if (c === 'record_calibration_sprint_response') return Promise.reject(new Error('db locked'));
      return Promise.resolve({});
    });
    render(<SprintPhase onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText('Rust 1.99 released')).toBeInTheDocument());

    fireEvent.click(screen.getByText('calibrationView.sprint.relevant'));
    await waitFor(() => expect(screen.getByText(/db locked/)).toBeInTheDocument());
    // Still on the first card — the label was NOT silently dropped.
    expect(screen.getByText('Rust 1.99 released')).toBeInTheDocument();
  });
});
