// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { RssFeedValidation, YouTubeChannelValidation } from '../../lib/commands';
import { cmd } from '../../lib/commands';
import { translateError } from '../../utils/error-messages';

export type ValidationResult = (RssFeedValidation & YouTubeChannelValidation) | null;

export const STATUS_CLEAR_DELAY = 2000;
export const VALIDATION_CLEAR_DELAY = 5000;
export const STATUS_ERROR_CLEAR_DELAY = 3000;

/**
 * Factory for toggle-default-source callbacks.
 * All three source types (RSS, YouTube, Twitter) follow the same pattern:
 * toggle an item in/out of the disabled list, then persist via a backend command.
 */
export function createToggleDefault(
  getDisabled: () => string[],
  setDisabled: (v: string[]) => void,
  cmdName: string,
  payloadKey: string,
  onError: (msg: string) => void,
) {
  return async (item: string, enabled: boolean) => {
    const updated = enabled
      ? getDisabled().filter((x) => x !== item)
      : [...getDisabled(), item];
    setDisabled(updated);
    try { await cmd(cmdName as never, { [payloadKey]: updated } as never); }
    catch (error) { onError(`Error: ${translateError(error)}`); }
  };
}
