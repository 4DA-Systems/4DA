// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { describe, it, expect } from 'vitest';
import {
  ABSTENTION_MARKER,
  isAbstentionSynthesis,
  parseAbstention,
  stripSynthesisTelemetry,
} from '../briefing-synthesis-helpers';

describe('isAbstentionSynthesis', () => {
  it('matches the exact Rust-emitted marker', () => {
    expect(isAbstentionSynthesis(ABSTENTION_MARKER)).toBe(true);
  });

  it('matches the marker with telemetry tail', () => {
    const input = `${ABSTENTION_MARKER}\n\n(25 items scanned, synthesis skipped: 4 ungrounded terms detected)`;
    expect(isAbstentionSynthesis(input)).toBe(true);
  });

  it('matches case-insensitively', () => {
    expect(isAbstentionSynthesis('LOW SIGNAL — NO NOTEWORTHY INTELLIGENCE OVERNIGHT.')).toBe(true);
  });

  it('matches with ASCII hyphen instead of em-dash', () => {
    expect(isAbstentionSynthesis('Low signal - no noteworthy intelligence overnight.')).toBe(true);
  });

  it('matches with en-dash', () => {
    expect(isAbstentionSynthesis('Low signal \u2013 no noteworthy intelligence overnight.')).toBe(true);
  });

  it('rejects a normal three-section briefing', () => {
    const normal = `SITUATION
Tokio released a security advisory [3].

PRIORITY
- Upgrade tokio to 1.38.5 for the RCE fix [3]
- Review TanStack Start RSC support [1]

PATTERN
Two of today's signals point at Rust async runtime stability.`;
    expect(isAbstentionSynthesis(normal)).toBe(false);
  });

  it('rejects empty string', () => {
    expect(isAbstentionSynthesis('')).toBe(false);
  });

  it('rejects null', () => {
    expect(isAbstentionSynthesis(null)).toBe(false);
  });

  it('rejects undefined', () => {
    expect(isAbstentionSynthesis(undefined)).toBe(false);
  });

  it('rejects whitespace-only input', () => {
    expect(isAbstentionSynthesis('   \n\n   ')).toBe(false);
  });

  it('rejects briefings that mention "low signal" in a later sentence', () => {
    // The marker is a line-1 literal. A briefing that happens to use
    // the phrase "low signal" further down must not be mis-classified
    // as abstention.
    const normal = `SITUATION
Multiple framework releases overnight.

PRIORITY
- None are critical; low signal on security front.`;
    expect(isAbstentionSynthesis(normal)).toBe(false);
  });
});

describe('parseAbstention', () => {
  it('splits headline and telemetry', () => {
    const input = `${ABSTENTION_MARKER}\n\n(25 items scanned, synthesis skipped: 4 ungrounded terms detected)`;
    const parsed = parseAbstention(input);
    expect(parsed.headline).toBe(ABSTENTION_MARKER);
    expect(parsed.telemetry).toContain('25 items scanned');
  });

  it('returns null telemetry when no tail is present', () => {
    const parsed = parseAbstention(ABSTENTION_MARKER);
    expect(parsed.headline).toBe(ABSTENTION_MARKER);
    expect(parsed.telemetry).toBeNull();
  });

  it('trims whitespace from both pieces', () => {
    const input = `  ${ABSTENTION_MARKER}  \n\n   (telemetry)   `;
    const parsed = parseAbstention(input);
    expect(parsed.headline).toBe(ABSTENTION_MARKER);
    expect(parsed.telemetry).toBe('(telemetry)');
  });
});

describe('stripSynthesisTelemetry', () => {
  it('removes telemetry tail from abstention output', () => {
    const input = `${ABSTENTION_MARKER}\n\n(telemetry bracket info)`;
    expect(stripSynthesisTelemetry(input)).toBe(ABSTENTION_MARKER);
  });

  it('leaves non-abstention synthesis unchanged', () => {
    const input = 'SITUATION\nReal briefing text with details.';
    expect(stripSynthesisTelemetry(input)).toBe(input);
  });
});
