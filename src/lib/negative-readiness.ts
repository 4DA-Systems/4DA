// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { AdvisorySync } from '../../src-tauri/bindings/bindings/AdvisorySync';
import type { DataFreshness } from '../../src-tauri/bindings/bindings/DataFreshness';

/**
 * When a "confident negative" may be said (Intelligence Doctrine rule 6).
 *
 * Preemption "your stack is clean", Knowledge Gaps "your knowledge is
 * current" and the Blind Spots empty states are claims. Each is true only
 * when the data that would contradict it was actually gathered — not when
 * some number of items has been fetched. The old gate (>= 100 fetched items
 * across enabled sources) dated from when news sources were always on; under
 * AD-054 a new user reads only stack sources, and a small project may never
 * reach 100 items, which left these panels silent forever.
 *
 * Real findings always render; these rules gate only the empty states.
 */

/** What an empty Preemption worklist may show (dependencies are known). */
export type PreemptionNegative =
  /** Every ecosystem synced recently and this feed was matched against it. */
  | { kind: 'assert' }
  /** A sync is downloading advisories, or ran after this feed was computed. */
  | { kind: 'checking' }
  /** An ecosystem was never synced and no sync is running (yet). */
  | { kind: 'not_started' }
  /** The latest advisory sync failed. */
  | { kind: 'failed'; error: string | null; inFlight: boolean }
  /** The advisory data is older than the backend's trust horizon. */
  | { kind: 'stale'; syncedAt: string | null; inFlight: boolean }
  /**
   * No dependency in an ecosystem the advisory database covers. `tempSkipped`
   * counts project folders the lockfile walk skipped because they are inside
   * the OS temp directory — then THAT is the reason, not coverage.
   */
  | { kind: 'uncovered'; tempSkipped: number }
  /** Readiness unknown (a backend without the field, or an unreadable DB). */
  | { kind: 'silent' };

/**
 * Preemption: "no known vulnerabilities" needs a read inventory whose every
 * ecosystem has a successful, recent advisory sync that this feed was matched
 * against — the backend's `advisory_sync`. Independent of which news sources
 * are on: interests never touch this claim.
 */
export function preemptionNegative(sync: AdvisorySync | null | undefined): PreemptionNegative {
  if (!sync) return { kind: 'silent' };
  switch (sync.state) {
    case 'ready':
      return { kind: 'assert' };
    case 'syncing':
    case 'matching':
      return { kind: 'checking' };
    case 'not_started':
      return { kind: 'not_started' };
    case 'failed':
      return { kind: 'failed', error: sync.error, inFlight: sync.in_flight === true };
    case 'stale':
      return { kind: 'stale', syncedAt: sync.synced_at, inFlight: sync.in_flight === true };
    case 'no_inventory':
      return { kind: 'uncovered', tempSkipped: sync.skipped_temp ?? 0 };
    default:
      return { kind: 'silent' };
  }
}

/**
 * Knowledge Gaps / Blind Spots: a negative about what you have read rests on
 * sources having been read. True when a source check has succeeded, the data
 * is not stale, and an engine cycle has completed (it links fetched items to
 * your dependencies). One stack-source fetch of any size is enough; there is
 * no item-count floor.
 */
export function sourceNewsReady(freshness: DataFreshness | null | undefined): boolean {
  if (!freshness) return false;
  return (
    !freshness.is_stale
    && freshness.newest_source_check_age_hours != null
    && freshness.source_checks_last_72h > 0
    && freshness.last_run_age_minutes != null
  );
}
