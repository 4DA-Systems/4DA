// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// The resolver places labels from a MODEL of their boxes (canvas text
// metrics, typography constants). This is the ground-truth check after it:
// read the rendered rects of every visible label and suppress the lower-
// priority one of any pair that still overlaps on screen — so a model/DOM
// drift (font hinting, a glyph the canvas measured narrower, a future CSS
// change) can never put text on text. Runs trailing, after zoom settles.

interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface RenderedLabel {
  id: string;
  priority: number;
  rect: Rect;
}

/** Overlap must exceed this on BOTH axes, screen px (anti-aliasing slack). */
const TOLERANCE_PX = 1;

function overlaps(a: Rect, b: Rect): boolean {
  const ox = Math.min(a.right, b.right) - Math.max(a.left, b.left);
  const oy = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
  return ox > TOLERANCE_PX && oy > TOLERANCE_PX;
}

/** Greedy keep-by-priority over RENDERED rects: a label overlapping a fixed
 *  rect (painted text no label may cover — the story badges) or a kept,
 *  higher-priority label is returned for suppression. Ties break by id. */
export function labelsToSuppress(labels: RenderedLabel[], fixed: Rect[] = []): string[] {
  const order = [...labels].sort((a, b) => b.priority - a.priority || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  const kept: Rect[] = [];
  const out: string[] = [];
  for (const l of order) {
    const pinned = l.priority === Infinity; // the lane header always stays
    if (!pinned && (fixed.some((f) => overlaps(f, l.rect)) || kept.some((k) => overlaps(k, l.rect)))) {
      out.push(l.id);
    } else {
      kept.push(l.rect);
    }
  }
  return out;
}

function isRendered(el: Element): boolean {
  if (el.getClientRects().length === 0) return false; // display:none (LOD, suppressed)
  return window.getComputedStyle(el).visibility !== 'hidden'; // a hovered node's own label
}

/** The painted text box: a node label's span is a fixed-width centred box,
 *  so its INK (a Range over the text) is what can collide, not the span. */
function inkRect(el: Element): Rect {
  if (typeof document.createRange === 'function') {
    const range = document.createRange();
    range.selectNodeContents(el);
    if (typeof range.getBoundingClientRect === 'function') {
      const r = range.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) return r;
    }
  }
  return el.getBoundingClientRect();
}

/** Measure every visible label on the page and suppress what still collides.
 *  Returns the ids it suppressed (empty when the model was right). */
export function verifyRenderedLabels(host: ParentNode, priorities: Map<string, number>): string[] {
  const labels: RenderedLabel[] = [];
  const byId = new Map<string, HTMLElement>();
  host.querySelectorAll<HTMLElement>('[data-cg-label-id]').forEach((el) => {
    if (el.getAttribute('data-cg-suppressed') === 'true' || !isRendered(el)) return;
    const id = el.getAttribute('data-cg-label-id') ?? '';
    byId.set(id, el);
    labels.push({ id, priority: priorities.get(id) ?? 0, rect: inkRect(el) });
  });
  const fixed: Rect[] = [];
  host.querySelectorAll<HTMLElement>('.cg-node-badge').forEach((el) => {
    if (isRendered(el)) fixed.push(el.getBoundingClientRect());
  });
  const suppress = labelsToSuppress(labels, fixed);
  for (const id of suppress) byId.get(id)?.setAttribute('data-cg-suppressed', 'true');
  return suppress;
}
