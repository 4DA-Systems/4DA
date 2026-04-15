/**
 * Phase 6a — classify interaction pattern from dwell time.
 *
 * These tests pin the thresholds between Bounced / Scanned / Engaged /
 * Abandoned. Changing the bands is a strategic scoring decision; the test
 * makes the change visible and intentional rather than silent.
 */
import { describe, it, expect } from 'vitest';
import { classifyInteractionPattern } from './use-expand-tracking';

describe('classifyInteractionPattern', () => {
  it('classifies a sub-4-second dwell as bounced', () => {
    expect(classifyInteractionPattern(0)).toBe('bounced');
    expect(classifyInteractionPattern(1)).toBe('bounced');
    expect(classifyInteractionPattern(3)).toBe('bounced');
  });

  it('classifies 4-19 seconds as scanned', () => {
    expect(classifyInteractionPattern(4)).toBe('scanned');
    expect(classifyInteractionPattern(10)).toBe('scanned');
    expect(classifyInteractionPattern(19)).toBe('scanned');
  });

  it('classifies 20-120 seconds as engaged', () => {
    expect(classifyInteractionPattern(20)).toBe('engaged');
    expect(classifyInteractionPattern(45)).toBe('engaged');
    expect(classifyInteractionPattern(120)).toBe('engaged');
  });

  it('classifies dwell > 120 seconds as abandoned (tab left open)', () => {
    // Load-bearing: a 10-minute dwell with no other signals is NOT
    // engagement — it's a forgotten tab. We will not reward it as positive.
    expect(classifyInteractionPattern(121)).toBe('abandoned');
    expect(classifyInteractionPattern(600)).toBe('abandoned');
    expect(classifyInteractionPattern(3600)).toBe('abandoned');
  });

  it('is deterministic (same input → same pattern)', () => {
    for (const d of [0, 3, 4, 19, 20, 120, 121, 600]) {
      expect(classifyInteractionPattern(d)).toBe(classifyInteractionPattern(d));
    }
  });
});
