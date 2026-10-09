// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { installRafHarness, type RafHarness } from '../test/motion-test-utils';
import {
  isAmbientMotionAllowed,
  isMotionAllowed,
  isWindowFocused,
  subscribeMotionGate,
} from './motion-gate';

describe('motion gate', () => {
  let h: RafHarness;
  beforeEach(() => {
    h = installRafHarness();
  });
  afterEach(() => h.restore());

  it('loop motion ignores focus (use-void-signals contract); ambient motion needs it', () => {
    expect(isMotionAllowed()).toBe(true);
    expect(isAmbientMotionAllowed()).toBe(true);
    h.setFocused(false);
    expect(isWindowFocused()).toBe(false);
    expect(isMotionAllowed()).toBe(true);
    expect(isAmbientMotionAllowed()).toBe(false);
  });

  it('ambient motion stops when hidden or under reduced motion, even if focused', () => {
    h.setHidden(true);
    expect(isAmbientMotionAllowed()).toBe(false);
    h.setHidden(false);
    h.setReducedMotion(true);
    expect(isAmbientMotionAllowed()).toBe(false);
  });

  it('notifies on visibility, focus, blur and reduced-motion changes, and unsubscribes', () => {
    const onChange = vi.fn();
    const unsubscribe = subscribeMotionGate(onChange);
    h.setFocused(false);
    h.setFocused(true);
    h.setHidden(true);
    h.setReducedMotion(true);
    expect(onChange).toHaveBeenCalledTimes(4);
    unsubscribe();
    h.setFocused(false);
    h.setHidden(false);
    expect(onChange).toHaveBeenCalledTimes(4);
  });
});
