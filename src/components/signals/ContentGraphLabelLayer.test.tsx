// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { StrictMode } from 'react';
import { render, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// The layer reads only zoom + nodes from React Flow's store; a fixed fake
// store keeps this a test of the EFFECT WIRING, not of React Flow.
const fakeState = {
  transform: [0, 0, 0.2] as [number, number, number],
  nodes: [
    { id: 'a', type: 'contentNode', position: { x: 0, y: 0 }, data: { title: 'npm: openai v7.27.0', member_count: 1, relevance_score: 0.9, affects_you: true } },
    { id: 'b', type: 'contentNode', position: { x: 4, y: 2 }, data: { title: 'npm: stripe v23.0.0', member_count: 1, relevance_score: 0.8, affects_you: true } },
    { id: 'c', type: 'contentNode', position: { x: 300, y: 0 }, data: { title: 'plain item', member_count: 1, relevance_score: 0.5, affects_you: false } },
  ],
};
vi.mock('@xyflow/react', () => ({
  useStore: (selector: (s: typeof fakeState) => unknown) => selector(fakeState),
}));

import { LabelCollisionLayer } from './ContentGraphLabelLayer';

function Graph() {
  return (
    <div className="react-flow">
      <span data-cg-label-id="node:a">openai v7.27.0</span>
      <span data-cg-label-id="node:b">stripe v23.0.0</span>
      <span data-cg-label-id="node:c">plain item</span>
      <LabelCollisionLayer />
    </div>
  );
}

const label = (c: HTMLElement, id: string) => c.querySelector<HTMLElement>(`[data-cg-label-id="${id}"]`)!;

describe('LabelCollisionLayer effect wiring', () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ['requestAnimationFrame', 'cancelAnimationFrame', 'setTimeout', 'clearTimeout'] });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  // Live 2026-10-04: under StrictMode's mount → cleanup → remount the cleanup
  // cancelled the pending frame but left its id in the ref, so every later
  // effect returned early and NO label was ever placed (0 of 174).
  it('places labels after the first frame under React.StrictMode', () => {
    const { container } = render(
      <StrictMode>
        <Graph />
      </StrictMode>,
    );
    expect(label(container, 'node:a').hasAttribute('data-cg-suppressed')).toBe(false);
    act(() => {
      vi.advanceTimersByTime(50);
    });
    for (const id of ['node:a', 'node:b']) {
      expect(label(container, id).getAttribute('data-cg-suppressed')).toMatch(/^(true|false)$/);
      expect(label(container, id).getAttribute('data-cg-place')).toBeTruthy();
    }
    // Two stack labels on one spot: they never both keep the default spot.
    const a = label(container, 'node:a');
    const b = label(container, 'node:b');
    const bothBelow =
      a.dataset.cgSuppressed === 'false' && b.dataset.cgSuppressed === 'false' &&
      a.dataset.cgPlace === 'below' && b.dataset.cgPlace === 'below';
    expect(bothBelow).toBe(false);
    // Far zoom: the non-stack label is LOD-hidden, left at its default.
    expect(label(container, 'node:c').dataset.cgPlace).toBe('below');
  });

  it('re-places on a zoom change after the first pass', () => {
    const { container, rerender } = render(
      <StrictMode>
        <Graph />
      </StrictMode>,
    );
    act(() => {
      vi.advanceTimersByTime(50);
    });
    // A sentinel only a NEW pass overwrites.
    label(container, 'node:c').setAttribute('data-cg-place', 'stale');
    fakeState.transform = [0, 0, 1];
    rerender(
      <StrictMode>
        <Graph />
      </StrictMode>,
    );
    act(() => {
      vi.advanceTimersByTime(50);
    });
    expect(label(container, 'node:c').getAttribute('data-cg-place')).not.toBe('stale');
    fakeState.transform = [0, 0, 0.2];
  });
});
