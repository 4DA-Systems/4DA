// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { BlindSpotsAssessSection } from './BlindSpotsAssessSection';

const cmdMock = vi.fn((..._a: unknown[]) => Promise.resolve(undefined));
vi.mock('../../lib/commands', () => ({ cmd: (...a: unknown[]) => cmdMock(...(a as [string, unknown])) }));

beforeEach(() => cmdMock.mockClear());

describe('BlindSpotsAssessSection — auto-assess toggle', () => {
  it('reflects the initial enabled state', () => {
    render(<BlindSpotsAssessSection initialEnabled={true} />);
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
  });

  it('reflects the initial disabled state', () => {
    render(<BlindSpotsAssessSection initialEnabled={false} />);
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
  });

  it('toggles off and persists via set_auto_assess_blind_spots', async () => {
    render(<BlindSpotsAssessSection initialEnabled={true} />);
    fireEvent.click(screen.getByRole('switch'));
    await waitFor(() => expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'false'));
    expect(cmdMock).toHaveBeenCalledWith('set_auto_assess_blind_spots', { enabled: false });
  });

  it('toggles on and persists', async () => {
    render(<BlindSpotsAssessSection initialEnabled={false} />);
    fireEvent.click(screen.getByRole('switch'));
    await waitFor(() => expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true'));
    expect(cmdMock).toHaveBeenCalledWith('set_auto_assess_blind_spots', { enabled: true });
  });

  it('reverts the optimistic toggle if persistence fails', async () => {
    cmdMock.mockImplementationOnce(() => Promise.reject(new Error('boom')));
    render(<BlindSpotsAssessSection initialEnabled={true} />);
    fireEvent.click(screen.getByRole('switch'));
    // optimistic flip then revert back to true
    await waitFor(() => expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true'));
  });
});
