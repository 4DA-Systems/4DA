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

  // A command returning `FourDaError` (get_knowledge_gaps, the standing-query
  // commands, ...) rejects with an OBJECT: the gate sentence is the `detail`
  // of the catch-all arm (error.rs). Looking for it only in a string showed
  // every free user "Couldn't check... Retry" instead of the upgrade card.
  it('detects the gate in the structured object the backend rejects with', () => {
    for (const detail of GATE_MESSAGES) {
      const rejection = {
        code: 'E9999',
        title: 'Unexpected error',
        detail,
        remediation: ['Try the operation again.', 'If this persists, restart 4DA.'],
        severity: 'error',
      };
      expect(isSignalGateError(rejection)).toBe(true);
    }
    expect(isSignalGateError({
      code: 'E3001',
      title: 'Database busy',
      detail: 'The database is temporarily locked by another operation.',
      remediation: ['Wait a moment and try again.'],
      severity: 'warning',
    })).toBe(false);
  });

  // A gate that reaches a surface with no upgrade card used to read "Something
  // went wrong" (string) or "Unexpected error: ... Try the operation again."
  // (object). It is named instead, without the backend sentence's free-trial
  // offer: the trial starts by itself on first launch and cannot be restarted.
  // Gated tabs still branch on isSignalGateError FIRST, to render the card.
  it('translateError names the gate rather than calling it a fault', () => {
    const out = translateError(GATE_MESSAGES[0]);
    expect(out).toBe('This is part of 4DA Signal. Upgrade to unlock it.');
    expect(out).not.toContain('requires 4DA Signal'); // not surfaced verbatim
    expect(out).not.toMatch(/free trial|try again/i);
    expect(isSignalGateError(GATE_MESSAGES[0])).toBe(true);
  });
});
