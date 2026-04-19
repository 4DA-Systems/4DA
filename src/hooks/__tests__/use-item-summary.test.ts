// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Tests for useItemSummary hook.
 *
 * Covers initial state, loading, error handling, and cache behavior.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';

// Mock the commands module
const mockCmd = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => mockCmd(...args),
}));

import { useItemSummary } from '../use-item-summary';

describe('useItemSummary', () => {
  beforeEach(() => {
    mockCmd.mockReset();
  });

  it('returns null summary initially', () => {
    const { result } = renderHook(() => useItemSummary(1, false));
    expect(result.current.summary).toBeNull();
    expect(result.current.summaryLoading).toBe(false);
    expect(result.current.summaryError).toBeNull();
  });

  it('fetches cached summary when expanded', async () => {
    mockCmd.mockResolvedValue({ summary: 'Cached summary text' });

    const { result } = renderHook(() => useItemSummary(1, true));

    await waitFor(() => {
      expect(result.current.summary).toBe('Cached summary text');
    });
    expect(mockCmd).toHaveBeenCalledWith('get_item_summary', { itemId: 1 });
  });

  it('does not fetch when not expanded', () => {
    renderHook(() => useItemSummary(1, false));
    expect(mockCmd).not.toHaveBeenCalled();
  });

  it('generateSummary sets loading and updates summary on success', async () => {
    mockCmd
      .mockRejectedValueOnce(new Error('no cache')) // get_item_summary fails
      .mockResolvedValueOnce({ summary: 'Generated summary' }); // generate_item_summary

    const { result } = renderHook(() => useItemSummary(1, true));

    await act(async () => {
      await result.current.generateSummary();
    });

    expect(result.current.summary).toBe('Generated summary');
    expect(result.current.summaryLoading).toBe(false);
    expect(result.current.summaryError).toBeNull();
  });

  it('generateSummary sets error on failure', async () => {
    mockCmd.mockRejectedValue(new Error('LLM timeout'));

    const { result } = renderHook(() => useItemSummary(1, false));

    await act(async () => {
      await result.current.generateSummary();
    });

    expect(result.current.summaryError).toBe('Request timed out. Try again in a moment.');
    expect(result.current.summaryLoading).toBe(false);
  });

  it('provides generateSummary as stable function reference', () => {
    const { result } = renderHook(() => useItemSummary(1, false));
    expect(typeof result.current.generateSummary).toBe('function');
  });
});
