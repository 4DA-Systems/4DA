// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// A failed update check is shown in the About panel, not swallowed (audit 2026-11-11, R5).
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { UpdateCheckStatus } from './UpdateCheckStatus';
import { __resetUpdateStatusForTests, registerUpdateCheck, setUpdateStatus } from '../hooks/update-status';

beforeEach(() => __resetUpdateStatusForTests());

describe('UpdateCheckStatus', () => {
  it('renders nothing when no updater is running (dev builds)', () => {
    const { container } = render(<UpdateCheckStatus />);
    expect(container).toBeEmptyDOMElement();
  });

  it('shows a failed check and runs a new one on "Check now"', () => {
    const run = vi.fn(() => Promise.resolve());
    registerUpdateCheck(run);
    setUpdateStatus({ lastCheckedAt: Date.now(), lastError: 'error sending request' });
    render(<UpdateCheckStatus />);
    expect(screen.getByText('about.updates.failed')).toBeInTheDocument();
    fireEvent.click(screen.getByText('about.updates.checkNow'));
    expect(run).toHaveBeenCalledTimes(1);
  });

  it('follows the status as a check runs and succeeds', () => {
    registerUpdateCheck(() => Promise.resolve());
    render(<UpdateCheckStatus />);
    expect(screen.getByText('about.updates.never')).toBeInTheDocument();
    act(() => setUpdateStatus({ checking: true }));
    expect(screen.getByText('about.updates.checking')).toBeInTheDocument();
    act(() => setUpdateStatus({ checking: false, lastCheckedAt: Date.now(), lastError: null }));
    expect(screen.getByText('about.updates.lastChecked')).toBeInTheDocument();
  });
});
