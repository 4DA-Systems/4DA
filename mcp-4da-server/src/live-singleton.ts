// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Live Intelligence Singleton
 *
 * Separated from index.ts to avoid circular dependency with tool-dispatch.ts.
 */

import type { LiveIntelligence } from "./live/index.js";

let instance: LiveIntelligence | null = null;

export function setLiveIntelligence(li: LiveIntelligence): void {
  instance = li;
}

export function getLiveIntelligence(): LiveIntelligence | null {
  return instance;
}
