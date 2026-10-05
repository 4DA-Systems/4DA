// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// One Tab-key trap for every modal dialog.
//
// The per-modal traps each re-implemented "wrap from last to first" and each
// leaked focus to the page behind the dialog in the same three ways:
//   - a disabled control counted as tabbable, so when the last button was
//     disabled (Save while saving, Finish while finishing) focus could never
//     be "on the last element" and Tab walked out of the dialog;
//   - a tabIndex=-1 element matched the bare `button` selector;
//   - focus already outside the dialog (a click on plain dialog text drops it
//     to <body>) was never pulled back, because the listener sat on the dialog.

const CANDIDATES = 'button, [href], input, select, textarea, [tabindex]';

/** The elements Tab can actually reach inside `root`, in DOM order. */
export function tabbableIn(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(CANDIDATES)).filter(
    (el) =>
      !el.hasAttribute('disabled') &&
      el.getAttribute('tabindex') !== '-1' &&
      el.getAttribute('aria-hidden') !== 'true' &&
      !el.closest('[hidden], [inert]') &&
      !(el instanceof HTMLInputElement && el.type === 'hidden'),
  );
}

/**
 * Keep Tab / Shift+Tab inside `root`. Call from a document-level keydown
 * listener so it also catches focus that has already left the dialog.
 */
export function trapTabKey(e: KeyboardEvent, root: HTMLElement | null): void {
  if (e.key !== 'Tab' || !root) return;
  // A dialog opened inside this one (Settings -> Team invite) owns Tab while
  // it is open; the outer trap stands down instead of fighting it.
  if (root.querySelector('[aria-modal="true"]')) return;
  const items = tabbableIn(root);
  if (items.length === 0) {
    e.preventDefault();
    return;
  }
  const first = items[0]!;
  const last = items[items.length - 1]!;
  const active = document.activeElement;
  const outside = !(active instanceof Node) || !root.contains(active);
  if (e.shiftKey) {
    if (outside || active === first) {
      e.preventDefault();
      last.focus();
    }
  } else if (outside || active === last) {
    e.preventDefault();
    first.focus();
  }
}
