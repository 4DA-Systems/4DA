// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

import { KeyboardShortcutsModal } from '../KeyboardShortcutsModal';

/**
 * Focus must never leave the shortcuts dialog while it is open. The trap used
 * to treat the tabIndex=-1 backdrop button (outside role="dialog") as the
 * first focusable element: Tab from the close button landed on the backdrop,
 * and Shift+Tab from there escaped to the page behind the modal.
 */
describe('KeyboardShortcutsModal focus trap', () => {
  function setup() {
    render(<KeyboardShortcutsModal onClose={vi.fn()} />);
    const dialog = screen.getByRole('dialog');
    const backdrop = document.querySelector<HTMLButtonElement>('button[tabindex="-1"]');
    if (!backdrop) throw new Error('backdrop button not rendered');
    const close = dialog.querySelector<HTMLButtonElement>('button');
    if (!close) throw new Error('close button not rendered');
    return { dialog, backdrop, close };
  }

  it('Tab from the last control wraps to a control inside the dialog, not the backdrop', () => {
    const { dialog, close } = setup();
    expect(document.activeElement).toBe(close);
    fireEvent.keyDown(close, { key: 'Tab' });
    expect(dialog.contains(document.activeElement)).toBe(true);
  });

  it('Shift+Tab from the first control wraps to a control inside the dialog', () => {
    const { dialog, close } = setup();
    fireEvent.keyDown(close, { key: 'Tab', shiftKey: true });
    expect(dialog.contains(document.activeElement)).toBe(true);
  });

  it('pulls focus back into the dialog when it sits on the backdrop', () => {
    const { dialog, backdrop } = setup();
    backdrop.focus();
    fireEvent.keyDown(backdrop, { key: 'Tab', shiftKey: true });
    expect(dialog.contains(document.activeElement)).toBe(true);
    backdrop.focus();
    fireEvent.keyDown(backdrop, { key: 'Tab' });
    expect(dialog.contains(document.activeElement)).toBe(true);
  });

  it('traps Tab even when focus has dropped to <body> (outside the modal wrapper)', () => {
    const { dialog, close } = setup();
    close.blur();
    expect(document.activeElement).toBe(document.body);
    fireEvent.keyDown(document.body, { key: 'Tab' });
    expect(dialog.contains(document.activeElement)).toBe(true);
  });
});
