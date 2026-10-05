// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach } from 'vitest';
import { tabbableIn, trapTabKey } from './focus-trap';

function tab(root: HTMLElement, shiftKey = false): KeyboardEvent {
  const e = new KeyboardEvent('keydown', { key: 'Tab', shiftKey, cancelable: true });
  trapTabKey(e, root);
  return e;
}

describe('focus trap', () => {
  let outsideButton: HTMLButtonElement;
  let dialog: HTMLDivElement;

  beforeEach(() => {
    document.body.innerHTML = `
      <button id="behind">behind the modal</button>
      <div id="dialog">
        <button id="backdrop" tabindex="-1">backdrop</button>
        <p>plain text</p>
        <input id="first" />
        <button id="middle">Middle</button>
        <button id="save" disabled>Save</button>
        <div hidden><button id="hidden-btn">hidden</button></div>
      </div>`;
    outsideButton = document.getElementById('behind') as HTMLButtonElement;
    dialog = document.getElementById('dialog') as HTMLDivElement;
  });

  it('counts only controls Tab can reach', () => {
    expect(tabbableIn(dialog).map((el) => el.id)).toEqual(['first', 'middle']);
  });

  it('wraps from the last reachable control even when the final button is disabled', () => {
    (document.getElementById('middle') as HTMLElement).focus();
    const e = tab(dialog);
    expect(e.defaultPrevented).toBe(true);
    expect(document.activeElement?.id).toBe('first');
  });

  it('Shift+Tab from the first control wraps to the last', () => {
    (document.getElementById('first') as HTMLElement).focus();
    tab(dialog, true);
    expect(document.activeElement?.id).toBe('middle');
  });

  it('pulls focus that has left the dialog back inside', () => {
    outsideButton.focus();
    const e = tab(dialog);
    expect(e.defaultPrevented).toBe(true);
    expect(document.activeElement?.id).toBe('first');
  });

  it('leaves Tab alone between two controls inside the dialog', () => {
    (document.getElementById('first') as HTMLElement).focus();
    const e = tab(dialog);
    expect(e.defaultPrevented).toBe(false);
  });

  it('an outer dialog stands down while a nested modal dialog is open', () => {
    const inner = document.createElement('div');
    inner.setAttribute('role', 'dialog');
    inner.setAttribute('aria-modal', 'true');
    inner.innerHTML = '<button id="inner-only">Inner</button>';
    dialog.appendChild(inner);
    (document.getElementById('inner-only') as HTMLElement).focus();

    const outer = tab(dialog);
    expect(outer.defaultPrevented).toBe(false);
    expect(document.activeElement?.id).toBe('inner-only');

    const own = tab(inner);
    expect(own.defaultPrevented).toBe(true);
    expect(document.activeElement?.id).toBe('inner-only');
  });

  it('ignores other keys and a missing dialog', () => {
    const e = new KeyboardEvent('keydown', { key: 'a', cancelable: true });
    trapTabKey(e, dialog);
    trapTabKey(new KeyboardEvent('keydown', { key: 'Tab', cancelable: true }), null);
    expect(e.defaultPrevented).toBe(false);
  });
});
