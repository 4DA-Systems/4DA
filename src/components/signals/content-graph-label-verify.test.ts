// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';

import { labelsToSuppress, verifyRenderedLabels } from './content-graph-label-verify';

const r = (left: number, top: number, w: number, h: number) => ({ left, top, right: left + w, bottom: top + h });

describe('labelsToSuppress (rendered-rect ground truth)', () => {
  it('suppresses the lower-priority label of an on-screen overlap', () => {
    expect(
      labelsToSuppress([
        { id: 'low', priority: 1, rect: r(10, 0, 60, 13) },
        { id: 'high', priority: 5, rect: r(0, 0, 60, 13) },
      ]),
    ).toEqual(['low']);
  });

  it('ignores sub-pixel contact (anti-aliasing slack)', () => {
    expect(
      labelsToSuppress([
        { id: 'a', priority: 2, rect: r(0, 0, 60, 13) },
        { id: 'b', priority: 1, rect: r(59.5, 12.5, 60, 13) },
      ]),
    ).toEqual([]);
  });

  it('suppresses a label over a story badge, but never the pinned lane header', () => {
    const badge = r(0, 0, 20, 16);
    expect(
      labelsToSuppress(
        [
          { id: 'node:x', priority: 1000, rect: r(5, 5, 60, 13) },
          { id: 'lane', priority: Infinity, rect: r(10, 2, 200, 22) },
        ],
        [badge],
      ),
    ).toEqual(['node:x']);
  });

  it('only a kept label blocks: a suppressed one frees its spot', () => {
    // b loses to a; c overlaps only b, so c stays.
    const out = labelsToSuppress([
      { id: 'a', priority: 3, rect: r(0, 0, 50, 13) },
      { id: 'b', priority: 2, rect: r(40, 0, 50, 13) },
      { id: 'c', priority: 1, rect: r(80, 0, 50, 13) },
    ]);
    expect(out).toEqual(['b']);
  });
});

describe('verifyRenderedLabels (DOM)', () => {
  function el(id: string, rect: ReturnType<typeof r>, extra: Record<string, string> = {}) {
    const span = document.createElement('span');
    span.setAttribute('data-cg-label-id', id);
    for (const [k, v] of Object.entries(extra)) span.setAttribute(k, v);
    span.textContent = id;
    span.getBoundingClientRect = () => ({ ...rect, x: rect.left, y: rect.top, width: rect.right - rect.left, height: rect.bottom - rect.top, toJSON: () => ({}) });
    span.getClientRects = () => [span.getBoundingClientRect()] as unknown as DOMRectList;
    return span;
  }

  it('writes data-cg-suppressed on what still collides on screen; skips hidden labels', () => {
    const host = document.createElement('div');
    const a = el('node:a', r(0, 0, 60, 13));
    const b = el('node:b', r(20, 2, 60, 13));
    const hidden = el('node:h', r(0, 0, 60, 13), { 'data-cg-suppressed': 'true' });
    host.append(a, b, hidden);
    const out = verifyRenderedLabels(host, new Map([['node:a', 2], ['node:b', 1], ['node:h', 9]]));
    expect(out).toEqual(['node:b']);
    expect(b.getAttribute('data-cg-suppressed')).toBe('true');
    expect(a.hasAttribute('data-cg-suppressed')).toBe(false);
  });
});
