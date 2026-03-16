import type { StateCreator } from 'zustand';
import type { AppStore, ToastSlice } from './types';

let toastId = 0;
const toastTimers = new Map<number, ReturnType<typeof setTimeout>>();

export const createToastSlice: StateCreator<AppStore, [], [], ToastSlice> = (set, get) => ({
  toasts: [],

  addToast: (type, message, action?) => {
    // Deduplicate: skip if a toast with the same message is already visible
    if (get().toasts.some(t => t.message === message)) {
      return;
    }

    const id = ++toastId;
    const duration = action ? 6000 : type === 'error' ? 8000 : 4000;

    set(state => {
      // Double-check inside set() to handle rapid concurrent calls
      if (state.toasts.some(t => t.message === message)) {
        return state;
      }
      const next = [...state.toasts, { id, type, message, action }];
      while (next.length > 3) {
        const removed = next.shift()!;
        const timer = toastTimers.get(removed.id);
        if (timer) {
          clearTimeout(timer);
          toastTimers.delete(removed.id);
        }
      }
      return { toasts: next };
    });

    const timer = setTimeout(() => {
      toastTimers.delete(id);
      set(state => ({ toasts: state.toasts.filter(t => t.id !== id) }));
    }, duration);
    toastTimers.set(id, timer);
  },

  removeToast: (id) => {
    const timer = toastTimers.get(id);
    if (timer) {
      clearTimeout(timer);
      toastTimers.delete(id);
    }
    set(state => ({ toasts: state.toasts.filter(t => t.id !== id) }));
  },
});
