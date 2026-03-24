/**
 * Tests for useToasts hook.
 *
 * Verifies that the hook provides correct store values and actions.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useToasts } from '../use-toasts';
import { useAppStore } from '../../store';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('useToasts', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns empty toasts array initially', () => {
    const { result } = renderHook(() => useToasts());
    expect(result.current.toasts).toEqual([]);
  });

  it('exposes addToast function', () => {
    const { result } = renderHook(() => useToasts());
    expect(typeof result.current.addToast).toBe('function');
  });

  it('exposes removeToast function', () => {
    const { result } = renderHook(() => useToasts());
    expect(typeof result.current.removeToast).toBe('function');
  });

  it('reflects toasts added via store', () => {
    const { result } = renderHook(() => useToasts());

    act(() => {
      result.current.addToast('success', 'Test message');
    });

    expect(result.current.toasts).toHaveLength(1);
    expect(result.current.toasts[0]!.message).toBe('Test message');
    expect(result.current.toasts[0]!.type).toBe('success');
  });

  it('removes toast via removeToast', () => {
    const { result } = renderHook(() => useToasts());

    act(() => {
      result.current.addToast('info', 'Removable toast');
    });

    const id = result.current.toasts[0]!.id;

    act(() => {
      result.current.removeToast(id);
    });

    expect(result.current.toasts).toHaveLength(0);
  });

  it('handles multiple toast additions', () => {
    const { result } = renderHook(() => useToasts());

    act(() => {
      result.current.addToast('success', 'First');
      result.current.addToast('error', 'Second');
      result.current.addToast('warning', 'Third');
    });

    expect(result.current.toasts).toHaveLength(3);
  });
});
