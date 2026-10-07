// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { StrictMode } from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, act } from '@testing-library/react';
import { installRafHarness, type RafHarness } from '../../test/motion-test-utils';
import { BrandMark } from './BrandMark';
import { frameIntervalMs } from './brand-mark-geometry';

describe('BrandMark animation loop', () => {
  let raf: RafHarness;

  beforeEach(() => {
    vi.useFakeTimers();
    raf = installRafHarness();
  });
  afterEach(() => {
    raf.restore();
    vi.useRealTimers();
  });

  /** Run one frame then let its throttle timer schedule the next. */
  const step = (time: number) => {
    act(() => raf.flush(time));
    act(() => {
      vi.advanceTimersByTime(200);
    });
  };

  it('draws a static first frame (geometry present before any rAF runs)', () => {
    const { container } = render(<BrandMark size={36} />);
    const circles = container.querySelectorAll('circle');
    expect(circles).toHaveLength(4);
    circles.forEach((c) => expect(c.getAttribute('cx')).toMatch(/^\d/));
    expect(container.querySelectorAll('polygon')[0]?.getAttribute('points')).toMatch(/,/);
  });

  it('rotates by mutating SVG attributes, not by re-rendering', () => {
    const { container } = render(<BrandMark size={36} />);
    expect(raf.pending()).toBe(1);
    const firstCircle = container.querySelector('circle')!;
    const before = firstCircle.getAttribute('cx');
    for (let t = 1; t <= 20; t++) step(t * 200);
    expect(firstCircle.isConnected).toBe(true); // same node, never replaced
    expect(firstCircle.getAttribute('cx')).not.toBe(before);
    expect(raf.pending()).toBe(1); // one loop, still running
  });

  it('schedules no frame while the document is hidden', () => {
    raf.setHidden(true);
    render(<BrandMark size={36} />);
    expect(raf.pending()).toBe(0);
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(raf.pending()).toBe(0);
  });

  it('stops when hidden and resumes when shown again', () => {
    render(<BrandMark size={36} />);
    expect(raf.pending()).toBe(1);
    act(() => raf.setHidden(true));
    expect(raf.pending()).toBe(0);
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(raf.pending()).toBe(0);
    act(() => raf.setHidden(false));
    expect(raf.pending()).toBe(1);
  });

  it('schedules no frame under prefers-reduced-motion, but renders a static frame', () => {
    raf.setReducedMotion(true);
    const { container } = render(<BrandMark size={36} />);
    expect(raf.pending()).toBe(0);
    expect(container.querySelector('circle')?.getAttribute('cx')).toMatch(/^\d/);
  });

  it('leaves exactly one loop running under StrictMode double-mount', () => {
    render(
      <StrictMode>
        <BrandMark size={36} />
      </StrictMode>,
    );
    expect(raf.pending()).toBe(1);
    for (let t = 1; t <= 5; t++) step(t * 200);
    expect(raf.pending()).toBe(1);
  });

  it('cancels its frame and timer on unmount', () => {
    const { unmount } = render(<BrandMark size={36} />);
    step(200);
    unmount();
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(raf.pending()).toBe(0);
  });

  it('uses the CSS breath animation instead of a re-render timer', () => {
    const { container } = render(<BrandMark size={36} />);
    expect(container.querySelector('svg.brand-mark-svg')).not.toBeNull();
    expect(container.querySelector('g.brand-mark-glow')).not.toBeNull();
    // No timers pending while no frame is in flight (no 10fps setInterval).
    raf.setHidden(true);
    expect(vi.getTimerCount()).toBe(0);
  });
});

describe('frameIntervalMs', () => {
  it('caps the step rate between 8 and 30 fps', () => {
    expect(frameIntervalMs(36, (2 * Math.PI) / 1800)).toBeCloseTo(125); // slow idle -> 8 fps
    expect(frameIntervalMs(400, 0.5)).toBeCloseTo(1000 / 30); // fast/large -> 30 fps cap
    for (const speed of [0.001, 0.0035, 0.014, 0.1]) {
      const ms = frameIntervalMs(36, speed);
      expect(ms).toBeGreaterThanOrEqual(1000 / 30 - 1e-9);
      expect(ms).toBeLessThanOrEqual(125 + 1e-9);
    }
  });
});
