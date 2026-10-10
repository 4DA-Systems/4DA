// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { createElement } from 'react';
import { deriveSignalVisuals } from './signal-visuals';
import { BrandMark } from './BrandMark';
import type { VoidSignal } from '../../types';

function signal(overrides: Partial<VoidSignal> = {}): VoidSignal {
  return {
    pulse: 0, heat: 0, burst: 0, morph: 0, error: 0, staleness: 0, item_count: 1502,
    signal_intensity: 0, signal_urgency: 0, critical_count: 0, signal_color_shift: 0,
    metabolism: 0, open_windows: 0, advantage_trend: 0,
    ...overrides,
  };
}

// Fresh-profile E2E 2026-10-10: the header mark rested (no fetch in flight)
// while its label read "Scanning" — a decayed pulse from the last fetch is not
// evidence of work. The label follows the same `active` signal as the motion.
describe('brand-mark label follows activity, not the raw pulse', () => {
  it('says Scanning while a fetch is in flight', () => {
    expect(deriveSignalVisuals(signal({ pulse: 0.9 }), false, true).stateLabel).toBe('Scanning');
  });

  it('does not say Scanning at rest, even with a stale high pulse', () => {
    expect(deriveSignalVisuals(signal({ pulse: 0.9 }), false, false).stateLabel).toBe('Active');
  });

  it('BrandMark at rest is labelled by its resting state', () => {
    const { container } = render(createElement(BrandMark, { signal: signal({ pulse: 0.9 }), active: false }));
    const mark = container.querySelector('.brand-mark-container')!;
    expect(mark.getAttribute('data-motion')).toBe('rest');
    expect(mark.getAttribute('aria-label')).not.toContain('Scanning');
  });
});
