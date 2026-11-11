// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Backend rejections as the frontend really receives them.
//
// `FourDaError` serialises as an object (src-tauri/src/error.rs `Serialize`
// -> `to_user_error`), so `String(e)` put "[object Object]" in front of users
// and the Signal gate was never recognised. Every fixture below is one arm of
// `to_user_error`, field for field.
import { describe, expect, it } from 'vitest';
import {
  budgetRefusalMessage,
  isSignalGateError,
  parseBudgetReset,
  rawErrorText,
  readableError,
  translateError,
  translateErrorWithRaw,
} from '../error-messages';

/** `FourDaError::Internal(msg)`: what every `Err("...".into())` becomes (catch-all arm). */
function internal(detail: string) {
  return {
    code: 'E9999',
    title: 'Unexpected error',
    detail,
    remediation: ['Try the operation again.', 'If this persists, restart 4DA.'],
    severity: 'error',
  };
}

/** `FourDaError::Validation(msg)`. */
function validation(detail: string) {
  return {
    code: 'E6001',
    title: 'Invalid input',
    detail,
    remediation: ['Check the input and try again.'],
    severity: 'warning',
  };
}

/** `require_signal_feature` through a command returning `FourDaError` (gating.rs). */
function gate(label: string) {
  return internal(`${label} requires 4DA Signal — start your free trial or upgrade to unlock it.`);
}

const DB_LOCKED = {
  code: 'E3001',
  title: 'Database busy',
  detail: 'The database is temporarily locked by another operation.',
  remediation: ['Wait a moment and try again.', 'If this persists, restart 4DA.'],
  severity: 'warning',
};

const LLM_NOT_CONFIGURED = {
  code: 'E5001',
  title: 'AI provider not configured',
  detail: 'No AI provider is set up for this operation.',
  remediation: [
    'Open Settings and configure an AI provider (OpenAI, Anthropic, or Ollama).',
    "For local AI: install Ollama and run 'ollama pull nomic-embed-text'.",
  ],
  severity: 'warning',
};

const NOT_INITIALIZED = {
  code: 'E1001',
  title: 'Component not ready',
  detail: 'ACE engine is still initializing.',
  remediation: ['Wait a moment for 4DA to finish starting up.', 'If this persists, restart the application.'],
  severity: 'warning',
};

const SIGNAL_SENTENCE = 'This is part of 4DA Signal. Upgrade to unlock it.';
const GENERIC = 'Something went wrong. Please try again.';

describe('readableError', () => {
  it('reads a pass-through structured error as the backend wrote it', () => {
    expect(readableError(internal('License key cannot be empty'))).toBe('License key cannot be empty');
    expect(readableError(internal('Too many activation attempts. Please wait a minute before trying again.')))
      .toBe('Too many activation attempts. Please wait a minute before trying again.');
    expect(readableError(validation('Query too long (maximum 5000 characters)')))
      .toBe('Query too long (maximum 5000 characters)');
  });

  it('drops the catch-all boilerplate ("Unexpected error", "Try the operation again.")', () => {
    const text = readableError(internal('Interest already exists'));
    expect(text).not.toContain('Unexpected error');
    expect(text).not.toContain('Try the operation again');
  });

  it('renders a specific code as translateError does', () => {
    expect(readableError(DB_LOCKED)).toBe('Database error. Try restarting the app.');
    expect(readableError(LLM_NOT_CONFIGURED)).toBe(
      'AI provider not configured: No AI provider is set up for this operation. ' +
      'Open Settings and configure an AI provider (OpenAI, Anthropic, or Ollama).',
    );
    expect(readableError(NOT_INITIALIZED)).toBe(
      'Component not ready: ACE engine is still initializing. Wait a moment for 4DA to finish starting up.',
    );
  });

  it('keeps an Error message and a string as they are', () => {
    expect(readableError(new Error('IPC timeout'))).toBe('IPC timeout');
    expect(readableError(new TypeError('fetch failed'))).toBe('fetch failed');
    expect(readableError('Feed returned no items')).toBe('Feed returned no items');
  });

  it('reads a structured error that arrived serialised in a string', () => {
    expect(readableError(JSON.stringify(internal('Team relay is not configured'))))
      .toBe('Team relay is not configured');
    expect(readableError(new Error(JSON.stringify(DB_LOCKED)))).toBe('Database error. Try restarting the app.');
  });

  it('says a Signal gate is one, without the free-trial offer the backend sentence carries', () => {
    for (const rejection of [gate('Knowledge Gaps'), gate('This feature'), gate('Trust Ledger').detail]) {
      const text = readableError(rejection);
      expect(text).toBe(SIGNAL_SENTENCE);
      expect(text).not.toMatch(/free trial/i);
    }
  });

  it('localizes a spent AI budget carried in a structured error, never the token', () => {
    const budget = internal('Daily AI budget reached — resets at 00:00 local time (in 3 h 12 min) [budget_reset:00:00:192].');
    expect(parseBudgetReset(budget)).toEqual({ time: '00:00', minutes: 192 });
    expect(budgetRefusalMessage(budget)).toBe('Daily AI budget reached — resets at 00:00 (in 192 minutes)');
    expect(readableError(budget)).toBe('Daily AI budget reached — resets at 00:00 (in 192 minutes)');
    expect(readableError(budget)).not.toContain('[budget_reset');
  });

  it('falls back when the rejection carries no text', () => {
    expect(readableError(null)).toBe(GENERIC);
    expect(readableError(undefined)).toBe(GENERIC);
    expect(readableError('')).toBe(GENERIC);
    expect(readableError({})).toBe(GENERIC);
    expect(readableError({ unrelated: 1 }, 'Could not load the score breakdown.'))
      .toBe('Could not load the score breakdown.');
    expect(readableError(internal(''))).toBe('Unexpected error');
  });

  it('reads other object shapes by their message', () => {
    expect(readableError({ message: 'plugin refused the path' })).toBe('plugin refused the path');
    expect(readableError({ detail: 'only a detail' })).toBe('only a detail');
    expect(readableError(404)).toBe('404');
  });

  it('never prints an object', () => {
    const shapes: unknown[] = [
      gate('Blind Spots'), internal('x'), validation('y'), DB_LOCKED, LLM_NOT_CONFIGURED, NOT_INITIALIZED,
      {}, { code: 7 }, { code: 'E1', title: 'only a title' }, [], [internal('in an array')], new Error(''),
      Object.create(null), () => 'fn', Symbol('s'), null, undefined, 0, false,
    ];
    for (const shape of shapes) {
      for (const text of [readableError(shape), translateError(shape), rawErrorText(shape)]) {
        expect(text).not.toContain('[object');
      }
      expect(readableError(shape).length).toBeGreaterThan(0);
      expect(translateError(shape).length).toBeGreaterThan(0);
    }
  });
});

describe('rawErrorText', () => {
  it('is the backend sentence for a pass-through code, title and detail for a specific one', () => {
    expect(rawErrorText(gate('Knowledge Gaps')))
      .toBe('Knowledge Gaps requires 4DA Signal — start your free trial or upgrade to unlock it.');
    expect(rawErrorText(DB_LOCKED)).toBe('Database busy: The database is temporarily locked by another operation.');
  });

  it('is empty when there is nothing to read', () => {
    expect(rawErrorText(null)).toBe('');
    expect(rawErrorText(undefined)).toBe('');
    expect(rawErrorText({})).toBe('');
  });

  it('gives translateErrorWithRaw a log line instead of "[object Object]"', () => {
    const [friendly, raw] = translateErrorWithRaw(internal('no such table: source_items'));
    expect(raw).toBe('no such table: source_items');
    expect(friendly).not.toContain('[object');
  });
});

describe('isSignalGateError with structured rejections', () => {
  // Every label `signal_feature_label` can produce (gating.rs).
  const LABELS = [
    'Blind Spots', 'Knowledge Gaps', 'Standing Queries', 'Semantic Shifts', 'Project Health',
    'Tech Convergence', 'Cross-Project Dependencies', 'Intelligence Report', 'Trust Ledger', 'This feature',
  ];

  it.each(LABELS)('detects the %s gate as the catch-all object a FourDaError command rejects with', (label) => {
    expect(isSignalGateError(gate(label))).toBe(true);
  });

  it('detects it serialised into a string or an Error message', () => {
    expect(isSignalGateError(JSON.stringify(gate('Knowledge Gaps')))).toBe(true);
    expect(isSignalGateError(new Error(JSON.stringify(gate('Knowledge Gaps'))))).toBe(true);
  });

  it('detects it in the title, should the backend give the gate its own code', () => {
    expect(isSignalGateError({ code: 'E7001', title: 'Knowledge Gaps requires 4DA Signal', detail: '' })).toBe(true);
  });

  it('does not take other structured faults for a paywall', () => {
    for (const fault of [DB_LOCKED, LLM_NOT_CONFIGURED, NOT_INITIALIZED, internal('database is locked'), validation('bad')]) {
      expect(isSignalGateError(fault)).toBe(false);
    }
    expect(isSignalGateError({})).toBe(false);
    expect(isSignalGateError({ detail: 42 })).toBe(false);
  });
});

describe('translateError with structured rejections', () => {
  it('names a gate instead of "Unexpected error ... Try the operation again."', () => {
    expect(translateError(gate('Semantic Shifts'))).toBe(SIGNAL_SENTENCE);
  });

  it('categorizes by the backend sentence, not by the catch-all title', () => {
    expect(translateError(internal('Analysis already running'))).toBe('Already in progress. Please wait for it to complete.');
    // "Invalid input" + "... key ..." used to read as an API-key failure.
    expect(translateError(validation('License key cannot be empty'))).toBe('License key cannot be empty');
  });

  it('reads an unrecognised pass-through error as the backend wrote it', () => {
    expect(translateError(internal('Maximum of 10 active standing queries reached')))
      .toBe('Maximum of 10 active standing queries reached');
  });
});
