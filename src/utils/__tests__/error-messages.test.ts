// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Tests for error message translation utility.
 *
 * Verifies that raw backend error strings are mapped to user-friendly messages,
 * covering all 13 error pattern categories plus fallbacks.
 */
import { describe, it, expect } from 'vitest';
import { translateError, translateErrorWithRaw } from '../error-messages';

describe('translateError', () => {
  // --- Network errors ---
  it('translates network fetch errors', () => {
    expect(translateError('failed to fetch: timeout')).toBe(
      'Network request failed. Check your internet connection.',
    );
  });

  it('translates reqwest errors', () => {
    expect(translateError('reqwest::Error { kind: Request }')).toBe(
      'Network request failed. Check your internet connection.',
    );
  });

  // --- Timeout ---
  it('translates timeout errors', () => {
    expect(translateError('Request timed out after 30s')).toBe(
      'Request timed out. Try again in a moment.',
    );
  });

  it('translates deadline exceeded', () => {
    expect(translateError('deadline has been exceeded')).toBe(
      'Request timed out. Try again in a moment.',
    );
  });

  // --- API key ---
  it('translates API key errors', () => {
    expect(translateError('Invalid api key provided')).toBe(
      'Authentication failed. Check your API key in Settings.',
    );
  });

  it('translates 401 unauthorized', () => {
    expect(translateError('HTTP 401 Unauthorized')).toBe(
      'Authentication failed. Check your API key in Settings.',
    );
  });

  // --- Rate limiting ---
  it('translates rate limit errors', () => {
    expect(translateError('Rate limit exceeded')).toBe(
      'Rate limit reached. Please wait a moment and try again.',
    );
  });

  it('translates 429 errors', () => {
    expect(translateError('HTTP 429 Too Many Requests')).toBe(
      'Rate limit reached. Please wait a moment and try again.',
    );
  });

  // --- Ollama ---
  it('translates ollama not running error', () => {
    expect(translateError('Ollama is not running on localhost')).toBe(
      'Ollama is not running. Start Ollama or set an API key in Settings.',
    );
  });

  it('translates general ollama errors', () => {
    expect(translateError('embedding failed: model not found')).toBe(
      'Embedding service unavailable. Check that Ollama is running.',
    );
  });

  // --- Database ---
  it('translates database errors', () => {
    expect(translateError('sqlite: database is locked')).toBe(
      'Database error. Try restarting the app.',
    );
  });

  // --- Permission ---
  it('translates permission denied errors', () => {
    expect(translateError('EACCES: permission denied')).toBe(
      'Permission denied. Check file permissions.',
    );
  });

  // --- Serialization ---
  it('translates JSON parse errors', () => {
    expect(translateError('serde_json::Error: expected value')).toBe(
      'Data format error. Try again or restart the app.',
    );
  });

  // --- Already running ---
  it('translates already in progress errors', () => {
    expect(translateError('Analysis already in progress')).toBe(
      'Already in progress. Please wait for it to complete.',
    );
  });

  // --- File not found ---
  it('translates file not found errors', () => {
    expect(translateError('ENOENT: no such file or directory')).toBe(
      'File not found. It may have been moved or deleted.',
    );
  });

  // --- Tauri IPC ---
  it('translates Tauri IPC errors', () => {
    expect(translateError('__TAURI__ invoke error')).toBe(
      'App communication error. Please restart the app.',
    );
  });

  // --- Git ---
  it('translates git errors', () => {
    expect(translateError('not a git repository')).toBe(
      'Git operation failed. Check the repository path.',
    );
  });

  // --- Connection refused (non-Ollama) ---
  it('translates connection refused (non-Ollama)', () => {
    expect(translateError('tcp connection refused ECONNREFUSED')).toBe(
      'Network request failed. Check your internet connection.',
    );
  });

  // --- Fallbacks ---
  it('returns fallback for unknown errors', () => {
    expect(translateError('some random error')).toBe(
      'Something went wrong. Please try again.',
    );
  });

  it('returns fallback for empty string', () => {
    expect(translateError('')).toBe('Something went wrong. Please try again.');
  });

  it('returns fallback for null/undefined', () => {
    expect(translateError(null)).toBe('Something went wrong. Please try again.');
    expect(translateError(undefined)).toBe('Something went wrong. Please try again.');
  });

  it('handles Error objects', () => {
    expect(translateError(new Error('failed to fetch data'))).toBe(
      'Network request failed. Check your internet connection.',
    );
  });
});

describe('translateErrorWithRaw', () => {
  it('returns tuple of user-friendly and raw message', () => {
    const [friendly, raw] = translateErrorWithRaw(new Error('Rate limit exceeded'));
    expect(friendly).toBe('Rate limit reached. Please wait a moment and try again.');
    expect(raw).toBe('Rate limit exceeded');
  });

  it('returns fallback friendly and empty raw for null', () => {
    const [friendly, raw] = translateErrorWithRaw(null);
    expect(friendly).toBe('Something went wrong. Please try again.');
    expect(raw).toBe('');
  });
});
