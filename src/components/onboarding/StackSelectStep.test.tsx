// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// A failed stack-list load must say so and offer a retry; an empty grid with
// no message reads as "4DA supports no stacks".

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

import { StackSelectStep } from './StackSelectStep';

const profile = {
  id: 'rust_systems', name: 'Rust Systems', core_tech: ['rust', 'tokio'],
  pain_point_count: 3, ecosystem_shift_count: 2,
};

describe('StackSelectStep', () => {
  beforeEach(() => { cmdMock.mockReset(); });

  it('shows a load failure with Retry (compact mode included), and Retry recovers', async () => {
    let failing = true;
    cmdMock.mockImplementation((command: string) => {
      if (failing) return Promise.reject('db busy');
      if (command === 'get_stack_profiles') return Promise.resolve([profile]);
      return Promise.resolve([]);
    });

    render(<StackSelectStep selected={[]} onSelectionChange={vi.fn()} compact />);
    await act(async () => {});

    expect(screen.getByRole('alert')).toHaveTextContent('onboarding.stack.loadFailed');

    failing = false;
    await act(async () => {
      fireEvent.click(screen.getByText('action.retry'));
    });

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.getByText('Rust Systems')).toBeInTheDocument();
  });

  // Fresh-profile E2E 2026-10-09: a weak match must be shown, not ticked.
  it('auto-ticks only detections at or above the bar', async () => {
    cmdMock.mockImplementation((command: string) => {
      if (command === 'get_stack_profiles') return Promise.resolve([profile]);
      return Promise.resolve([
        { profile_id: 'rust_systems', profile_name: 'Rust Systems', confidence: 0.45, matched_tech: [] },
        { profile_id: 'bootstrap_webdev', profile_name: 'Web', confidence: 0.2, matched_tech: [] },
      ]);
    });
    const onSelectionChange = vi.fn();
    render(<StackSelectStep selected={[]} onSelectionChange={onSelectionChange} />);
    await act(async () => {});
    expect(onSelectionChange).toHaveBeenCalledWith(['rust_systems']);
  });
});
