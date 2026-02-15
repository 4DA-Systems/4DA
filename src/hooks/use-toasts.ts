import { useAppStore } from '../store';

export type { ToastType, ToastAction, Toast } from '../store';

/**
 * Toast hook — thin wrapper around Zustand store.
 * All state and timer management lives in the store.
 */
export function useToasts() {
  const toasts = useAppStore(s => s.toasts);
  const addToast = useAppStore(s => s.addToast);
  const removeToast = useAppStore(s => s.removeToast);

  return { toasts, addToast, removeToast };
}
