// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { isSignalGateError, translateError } from './error-messages';

// Antibody AB-011 (display-contradicts-data): the Preemption and Blind Spots
// tabs both shipped "paywall rendered as a red error banner" independently.
// The cure is a single shared classifier. These tests pin the contract so a
// future gated tab — or a reworded backend gate — can't silently reintroduce it.
describe('isSignalGateError', () => {
  // The exact strings require_signal_feature() emits (gating.rs:102), one per
  // gated feature label. If the backend rewords this, these break loudly.
  const GATE_MESSAGES = [
    'Preemption Radar requires 4DA Signal — start your free trial or upgrade to unlock it.',
    'Blind Spots requires 4DA Signal — start your free trial or upgrade to unlock it.',
    'Knowledge Gaps requires 4DA Signal — start your free trial or upgrade to unlock it.',
    'Signal Chains requires 4DA Signal — start your free trial or upgrade to unlock it.',
  ];

  it('detects every Signal-gated feature rejection', () => {
    for (const msg of GATE_MESSAGES) {
      expect(isSignalGateError(msg)).toBe(true);
    }
  });

  it('detects the gate when wrapped in an Error instance', () => {
    expect(isSignalGateError(new Error(GATE_MESSAGES[0]))).toBe(true);
  });

  it('is case-insensitive on the gate token', () => {
    expect(isSignalGateError('PREEMPTION RADAR REQUIRES 4DA SIGNAL — ...')).toBe(true);
  });

  it('does NOT classify genuine faults as a paywall', () => {
    const faults = [
      'Failed to fetch: reqwest::Error { kind: Request }',
      'Request timed out',
      'database is locked',
      'Something went wrong. Please try again.',
      'Authentication failed. Check your API key.',
    ];
    for (const f of faults) {
      expect(isSignalGateError(f)).toBe(false);
    }
  });

  it('does NOT match a generic mention of "Signal" (the paid tier name) without the gate phrase', () => {
    expect(isSignalGateError('Upgrade to Signal for more features')).toBe(false);
    expect(isSignalGateError('signal strength low')).toBe(false);
  });

  it('returns false for empty / nullish errors', () => {
    expect(isSignalGateError('')).toBe(false);
    expect(isSignalGateError(null)).toBe(false);
    expect(isSignalGateError(undefined)).toBe(false);
  });

  // The reason a dedicated classifier is needed: translateError has no Signal
  // pattern, so a paywall would fall through to the generic fallback and render
  // as a red error. This guards the regression at its root.
  it('translateError alone would mis-handle the gate (hence the separate classifier)', () => {
    const out = translateError(GATE_MESSAGES[0]);
    expect(out).not.toContain('requires 4DA Signal'); // not surfaced verbatim
    // It collapses to the generic fallback — which is exactly why callers must
    // branch on isSignalGateError BEFORE translateError.
    expect(isSignalGateError(GATE_MESSAGES[0])).toBe(true);
  });
});
