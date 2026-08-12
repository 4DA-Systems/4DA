// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useAppStore } from '../store';

// Single component-facing path for the toast types; they are defined once in
// `../store/types` and re-exported here only (not via the store or hooks barrels).
export type { ToastType, ToastAction, Toast } from '../store/types';

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
