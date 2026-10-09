// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { StrictMode } from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, act } from '@testing-library/react';
import { installRafHarness, type RafHarness } from '../../test/motion-test-utils';
import { BrandMark } from './BrandMark';
import {
  FRAME_COUNT,
  LOOP_ANGLE,
  SPRITE_COLS,
  SPRITE_ROWS,
  computeGeometry,
  frameAttrs,
  loopDurationMs,
  spriteFrames,
} from './brand-mark-geometry';
import type { BrandMarkFrame } from './brand-mark-geometry';
import type { VoidSignal } from '../../types';
import { BRAND_MARK_CSS as CSS } from './brand-mark-css';


function signal(overrides: Partial<VoidSignal> = {}): VoidSignal {
  return {
    pulse: 0, heat: 0, burst: 0, morph: 0, error: 0, staleness: 0, item_count: 0,
    signal_intensity: 0, signal_urgency: 0, critical_count: 0, signal_color_shift: 0,
    metabolism: 0, open_windows: 0, advantage_trend: 0,
    ...overrides,
  };
}

describe('BrandMark runs with zero JavaScript per frame', () => {
  let raf: RafHarness;

  beforeEach(() => {
    vi.useFakeTimers();
    raf = installRafHarness();
  });
  afterEach(() => {
    raf.restore();
    vi.useRealTimers();
  });

  it('schedules no requestAnimationFrame and no timer in steady state', () => {
    render(
      <StrictMode>
        <BrandMark signal={signal({ pulse: 0.8, item_count: 4 })} size={36} />
      </StrictMode>,
    );
    act(() => {
      vi.advanceTimersByTime(10_000);
    });
    expect(window.requestAnimationFrame).not.toHaveBeenCalled();
    expect(raf.pending()).toBe(0);
    expect(vi.getTimerCount()).toBe(0);
  });

  it('never rewrites geometry after mount (a signal change only moves CSS variables)', () => {
    const { container, rerender } = render(<BrandMark signal={signal()} size={36} />);
    const firstPolygon = container.querySelector('polygon')!;
    const points = firstPolygon.getAttribute('points');
    const status = container.querySelector('.brand-mark-container') as HTMLElement;
    const idleLoop = status.style.getPropertyValue('--bm-loop');

    rerender(<BrandMark signal={signal({ pulse: 0.8, critical_count: 1 })} size={36} />);
    expect(firstPolygon.isConnected).toBe(true);
    expect(firstPolygon.getAttribute('points')).toBe(points);
    expect(status.style.getPropertyValue('--bm-loop')).not.toBe(idleLoop);
    expect(status.style.getPropertyValue('--bm-edge')).toBe('#EF4444');
  });

  it('pauses while the document is hidden and resumes when shown (event-driven)', () => {
    const { container } = render(<BrandMark size={36} />);
    const status = container.querySelector('.brand-mark-container')!;
    expect(status.getAttribute('data-motion')).toBe('running');
    act(() => raf.setHidden(true));
    expect(status.getAttribute('data-motion')).toBe('paused');
    act(() => raf.setHidden(false));
    expect(status.getAttribute('data-motion')).toBe('running');
    expect(raf.pending()).toBe(0);
  });

  it('rests with no animation at all while 4DA is idle (active=false)', () => {
    const { container } = render(<BrandMark size={36} active={false} />);
    const status = container.querySelector('.brand-mark-container')!;
    expect(status.getAttribute('data-motion')).toBe('rest');
    // Focus/visibility changes never wake an idle mark.
    act(() => raf.setFocused(false));
    act(() => raf.setFocused(true));
    act(() => raf.setHidden(true));
    act(() => raf.setHidden(false));
    expect(status.getAttribute('data-motion')).toBe('rest');
    // `rest` REMOVES the animations (not merely pauses them), for all three layers.
    const rest = CSS.slice(CSS.indexOf('[data-motion="rest"]'));
    const block = rest.slice(0, rest.indexOf('}') + 1);
    for (const layer of ['viewport', 'sprite', 'glow']) {
      expect(block).toContain(`[data-motion="rest"] .brand-mark-${layer}`);
    }
    expect(block).toMatch(/animation: none;/);
    expect(window.requestAnimationFrame).not.toHaveBeenCalled();
  });

  it('runs while working, and goes back to rest when the work ends', () => {
    const { container, rerender } = render(<BrandMark size={36} active />);
    const status = container.querySelector('.brand-mark-container')!;
    expect(status.getAttribute('data-motion')).toBe('running');
    rerender(<BrandMark size={36} active={false} />);
    expect(status.getAttribute('data-motion')).toBe('rest');
    rerender(<BrandMark size={36} active />);
    expect(status.getAttribute('data-motion')).toBe('running');
  });

  it('pauses while the window is unfocused and resumes on focus (event-driven)', () => {
    const { container } = render(<BrandMark size={36} active />);
    const status = container.querySelector('.brand-mark-container')!;
    expect(status.getAttribute('data-motion')).toBe('running');
    act(() => raf.setFocused(false));
    expect(status.getAttribute('data-motion')).toBe('paused');
    act(() => raf.setFocused(true));
    expect(status.getAttribute('data-motion')).toBe('running');
    expect(raf.pending()).toBe(0);
  });

  it('starts paused when mounted into an unfocused window', () => {
    raf.setFocused(false);
    const { container } = render(<BrandMark size={36} active />);
    expect(container.querySelector('.brand-mark-container')!.getAttribute('data-motion')).toBe(
      'paused',
    );
  });

  it('renders a static first frame under prefers-reduced-motion', () => {
    raf.setReducedMotion(true);
    const { container } = render(<BrandMark size={36} />);
    expect(container.querySelector('.brand-mark-container')!.getAttribute('data-motion')).toBe(
      'paused',
    );
    const first = container.querySelector('.brand-mark-sharp [data-frame="0"]')!;
    expect(first.querySelectorAll('circle')[0]!.getAttribute('cx')).toMatch(/^\d/);
    // The sprite's resting transform is frame 0 and CSS removes the animations.
    expect(CSS).toMatch(
      /@media \(prefers-reduced-motion: reduce\)\s*{[^}]*\.brand-mark-sprite[^}]*animation-name: none !important/,
    );
    expect(window.requestAnimationFrame).not.toHaveBeenCalled();
  });
});

describe('BrandMark sprite structure', () => {
  it('renders one static sheet of FRAME_COUNT frames for the sharp and the glow layers', () => {
    const { container } = render(<BrandMark size={36} />);
    const sharp = container.querySelectorAll('.brand-mark-sharp > svg[data-frame]');
    const glow = container.querySelectorAll('.brand-mark-glow > svg[data-frame]');
    expect(sharp).toHaveLength(FRAME_COUNT);
    expect(glow).toHaveLength(FRAME_COUNT);
    sharp.forEach((frame) => {
      expect(frame.querySelectorAll('polygon')).toHaveLength(4);
      expect(frame.querySelectorAll('line')).toHaveLength(6);
      expect(frame.querySelectorAll('circle')).toHaveLength(4);
    });
    glow.forEach((frame) => expect(frame.querySelectorAll('line')).toHaveLength(6));
    const sprite = container.querySelector('[data-testid="brand-mark-sprite"]') as HTMLElement;
    expect(sprite.style.width).toBe(`${SPRITE_COLS * 36}px`);
    expect(sprite.style.height).toBe(`${SPRITE_ROWS * 36}px`);
  });

  it('injects its stylesheet once into <head>, however many marks render', () => {
    render(
      <>
        <BrandMark size={36} />
        <BrandMark size={80} />
      </>,
    );
    const sheets = document.head.querySelectorAll('style[data-href="fourda-brand-mark"]');
    expect(sheets).toHaveLength(1);
    expect(sheets[0]!.textContent).toContain('@keyframes brand-mark-turn');
  });

  it('keeps the blur filter off the animated element (only the glow sheet filters)', () => {
    const { container } = render(<BrandMark size={36} />);
    const sprite = container.querySelector('.brand-mark-sprite')!;
    expect(sprite.getAttribute('filter')).toBeNull();
    expect(container.querySelectorAll('.brand-mark-sharp [filter]')).toHaveLength(0);
    expect(container.querySelectorAll('.brand-mark-glow g[filter]')).toHaveLength(FRAME_COUNT);
  });

  it('frame 0 is the tetrahedron at rest (snapshot of the static geometry)', () => {
    const f0 = spriteFrames(36)[0]!;
    expect(f0).toEqual(frameAttrs(computeGeometry(0), 48));
    expect(f0.verts.map((v) => [v.cx, v.cy])).toMatchInlineSnapshot(`
      [
        [
          35.26,
          51.04,
        ],
        [
          84.07,
          61.32,
        ],
        [
          50,
          12.57,
        ],
        [
          29.81,
          75.4,
        ],
      ]
    `);
  });

  it('a third of a turn loops back to frame 0 (3-fold symmetry, seamless loop)', () => {
    // The symmetry permutes which vertex is which, so compare the PICTURE:
    // each polygon's corners and each line's endpoints in a canonical order,
    // equal to the source vertices' 4-digit precision (0.01 of a 100-unit cell).
    type Pt = [number, number];
    const byX = (a: Pt, b: Pt) => a[0] - b[0] || a[1] - b[1];
    const nums = (f: BrandMarkFrame) => [
      ...f.faces.flatMap((p) => [
        p.opacity,
        ...p.points
          .split(' ')
          .map((xy) => xy.split(',').map(Number) as Pt)
          .sort(byX)
          .flat(),
      ]),
      ...[...f.glow, ...f.edges].flatMap((l) => [
        l.strokeWidth,
        l.opacity,
        ...([[l.x1, l.y1], [l.x2, l.y2]] as Pt[]).sort(byX).flat(),
      ]),
      ...f.verts.flatMap((v) => [v.cx, v.cy, v.r, v.opacity]),
    ];
    const wrapped = nums(frameAttrs(computeGeometry(LOOP_ANGLE), 48));
    const first = nums(spriteFrames(36)[0]!);
    expect(wrapped).toHaveLength(first.length);
    wrapped.forEach((n, i) => expect(Math.abs(n - first[i]!)).toBeLessThanOrEqual(0.011));
    // Consecutive frames move: the sheet is not FRAME_COUNT copies of one pose.
    expect(spriteFrames(36)[1]).not.toEqual(spriteFrames(36)[0]);
  });

  it('the CSS keyframes step through exactly FRAME_COUNT cells of the grid', () => {
    const block = CSS.slice(CSS.indexOf('@keyframes brand-mark-turn'));
    const stops = block.match(/^\s+[\d.]+% \{ transform: translate\([^)]*\); \}/gm) ?? [];
    expect(stops).toHaveLength(FRAME_COUNT + 1);
    const last = stops[FRAME_COUNT - 1]!;
    const col = (SPRITE_COLS - 1) / SPRITE_COLS;
    const row = (SPRITE_ROWS - 1) / SPRITE_ROWS;
    expect(last).toContain(`-${+(col * 100).toFixed(4)}%, -${+(row * 100).toFixed(4)}%`);
    expect(CSS).toMatch(/\.brand-mark-sprite\s*{[^}]*steps\(1, end\)/);
  });
});

describe('loopDurationMs', () => {
  it('maps the signal rotation speed to a third-of-a-turn loop', () => {
    expect(loopDurationMs((2 * Math.PI) / (60 * 30))).toBe(20_000); // 60 s turn
    expect(loopDurationMs((2 * Math.PI) / (18 * 30))).toBe(6_000); // 18 s turn
    expect(loopDurationMs(0.014)).toBe(5_000); // no-signal default
    expect(loopDurationMs(0)).toBeGreaterThan(0);
  });
});
