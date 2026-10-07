// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// One project-discovery run shared by every caller. React StrictMode mounts
// effects twice and onboarding, Settings and the briefing nudge can overlap;
// before this, the second caller got `already_running` back and read it as
// "done, nothing found".

import { cmd } from './commands';
import type { CommandResult } from './commands';

export type DiscoveryResult = CommandResult<'ace_auto_discover'>;

const RETRY_MS = 1500;
const MAX_ATTEMPTS = 80; // ~2 minutes of an older backend's "already running"

let inFlight: { key: string; promise: Promise<DiscoveryResult> } | null = null;

/** A run that did not happen because another one was in progress. */
export function isStillRunning(result: DiscoveryResult | null | undefined): boolean {
  return result?.status === 'already_running';
}

/**
 * Scan the confirmed folders (or, with no list, the default home folders).
 * Concurrent calls for the same folders share one promise. A backend answer
 * of `already_running` is never a result: the call waits and asks again.
 */
export function runDiscovery(dirs?: readonly string[]): Promise<DiscoveryResult> {
  const key = dirs ? JSON.stringify([...dirs].sort()) : '*';
  if (inFlight && inFlight.key === key) return inFlight.promise;

  const params = dirs ? { dirs: [...dirs] } : {};
  const promise = (async () => {
    for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt++) {
      const result = await cmd('ace_auto_discover', params);
      if (!isStillRunning(result)) return result;
      await new Promise(r => setTimeout(r, RETRY_MS));
    }
    throw new Error('Project discovery is still running');
  })();
  const entry = { key, promise };
  inFlight = entry;
  const clear = () => { if (inFlight === entry) inFlight = null; };
  promise.then(clear, clear);
  return promise;
}

/** Test seam: forget any in-flight run. */
export function resetDiscoveryForTests(): void {
  inFlight = null;
}
