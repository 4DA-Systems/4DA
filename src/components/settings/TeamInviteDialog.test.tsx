// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Escape in the Team invite dialog closes only the dialog. It used to reach the
// app-wide Escape handler on window too, which closed all of Settings.

import { describe, it, expect, vi, afterEach } from 'vitest';
import { render } from '@testing-library/react';

vi.mock('../../store', () => ({
  useAppStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ createInvite: vi.fn() }),
}));

import { TeamInviteDialog } from './TeamInviteDialog';

describe('TeamInviteDialog', () => {
  const appWide = vi.fn();
  afterEach(() => window.removeEventListener('keydown', appWide));

  it('Escape closes the dialog without reaching the app-wide handler', () => {
    window.addEventListener('keydown', appWide);
    const onClose = vi.fn();
    render(<TeamInviteDialog onClose={onClose} />);

    document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

    expect(onClose).toHaveBeenCalledTimes(1);
    expect(appWide).not.toHaveBeenCalled();
  });

  it('keeps Tab inside the dialog when focus has left it', () => {
    render(<TeamInviteDialog onClose={vi.fn()} />);
    (document.activeElement as HTMLElement | null)?.blur();

    const e = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
    document.body.dispatchEvent(e);

    expect(e.defaultPrevented).toBe(true);
    expect(document.querySelector('[role="dialog"]')?.contains(document.activeElement)).toBe(true);
  });
});
