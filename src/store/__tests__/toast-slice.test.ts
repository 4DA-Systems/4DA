import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('toast-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has empty toasts array', () => {
      expect(useAppStore.getState().toasts).toEqual([]);
    });
  });

  // ---------------------------------------------------------------------------
  // addToast
  // ---------------------------------------------------------------------------
  describe('addToast', () => {
    it('adds a success toast', () => {
      useAppStore.getState().addToast('success', 'Operation completed');
      const toasts = useAppStore.getState().toasts;
      expect(toasts).toHaveLength(1);
      expect(toasts[0]!.type).toBe('success');
      expect(toasts[0]!.message).toBe('Operation completed');
    });

    it('adds an error toast', () => {
      useAppStore.getState().addToast('error', 'Something went wrong');
      const toasts = useAppStore.getState().toasts;
      expect(toasts).toHaveLength(1);
      expect(toasts[0]!.type).toBe('error');
      expect(toasts[0]!.message).toBe('Something went wrong');
    });

    it('adds multiple toasts', () => {
      useAppStore.getState().addToast('success', 'First');
      useAppStore.getState().addToast('info', 'Second');
      useAppStore.getState().addToast('warning', 'Third');
      expect(useAppStore.getState().toasts).toHaveLength(3);
    });

    it('limits toasts to 3 maximum', () => {
      useAppStore.getState().addToast('success', 'First');
      useAppStore.getState().addToast('info', 'Second');
      useAppStore.getState().addToast('warning', 'Third');
      useAppStore.getState().addToast('error', 'Fourth');

      const toasts = useAppStore.getState().toasts;
      expect(toasts).toHaveLength(3);
      // The first toast should have been removed
      expect(toasts[0]!.message).toBe('Second');
      expect(toasts[2]!.message).toBe('Fourth');
    });

    it('adds toast with action', () => {
      const action = { label: 'Undo', onClick: vi.fn() };
      useAppStore.getState().addToast('info', 'Item dismissed', action);

      const toasts = useAppStore.getState().toasts;
      expect(toasts).toHaveLength(1);
      expect(toasts[0]!.action).toBeDefined();
      expect(toasts[0]!.action!.label).toBe('Undo');
    });

    it('auto-removes toast after duration', () => {
      useAppStore.getState().addToast('success', 'Auto-remove');
      expect(useAppStore.getState().toasts).toHaveLength(1);

      // Success toast duration is 4000ms
      vi.advanceTimersByTime(4000);
      expect(useAppStore.getState().toasts).toHaveLength(0);
    });

    it('error toast persists until dismissed (no auto-dismiss)', () => {
      useAppStore.getState().addToast('error', 'Error toast');
      expect(useAppStore.getState().toasts).toHaveLength(1);

      // Error toasts do NOT auto-dismiss — they persist for accessibility
      vi.advanceTimersByTime(10000);
      expect(useAppStore.getState().toasts).toHaveLength(1);

      // Must be manually dismissed
      const id = useAppStore.getState().toasts[0]!.id;
      useAppStore.getState().removeToast(id);
      expect(useAppStore.getState().toasts).toHaveLength(0);
    });
  });

  // ---------------------------------------------------------------------------
  // removeToast
  // ---------------------------------------------------------------------------
  describe('removeToast', () => {
    it('removes a toast by id', () => {
      useAppStore.getState().addToast('success', 'To remove');
      const toastId = useAppStore.getState().toasts[0]!.id;

      useAppStore.getState().removeToast(toastId);
      expect(useAppStore.getState().toasts).toHaveLength(0);
    });

    it('only removes the specified toast', () => {
      useAppStore.getState().addToast('success', 'Keep');
      useAppStore.getState().addToast('error', 'Remove');

      const toasts = useAppStore.getState().toasts;
      const removeId = toasts[1]!.id;

      useAppStore.getState().removeToast(removeId);

      const remaining = useAppStore.getState().toasts;
      expect(remaining).toHaveLength(1);
      expect(remaining[0]!.message).toBe('Keep');
    });

    it('does nothing when removing a non-existent id', () => {
      useAppStore.getState().addToast('info', 'Stays');
      useAppStore.getState().removeToast(99999);
      expect(useAppStore.getState().toasts).toHaveLength(1);
    });
  });
});
