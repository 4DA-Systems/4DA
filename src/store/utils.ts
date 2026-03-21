import { useAppStore } from './index';
import { translateError } from '../utils/error-messages';

/**
 * Surface a store-level error to the user via toast notification.
 * Use for user-triggered actions that fail silently. Do NOT use for
 * background tasks (telemetry, cache pruning, tracking) — those should stay silent.
 */
export function surfaceError(error: unknown, context: string): void {
  const message = translateError(error);
  console.error(`[4DA] ${context}:`, error);
  try {
    useAppStore.getState().addToast('error', `${context}: ${message}`);
  } catch {
    // Store not initialized yet — console.error above already logged it
  }
}
