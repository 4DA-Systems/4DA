/**
 * Refresh adapters (S3) — wrap each named data pull into a schedulable refresh unit.
 *
 * Pure dependency injection: the pulls and the freshness sink are handed in, nothing is
 * imported from the fetcher layer and nothing touches the network here. A refresh unit runs
 * its pull and only THEN marks freshness — a failing pull rethrows and leaves the watermark
 * untouched, so staleness stays honest.
 */

export interface RefreshDeps {
  pull: Record<string, () => Promise<void>>;
  freshness: { markFresh(name: string, at?: number): void };
}

export interface RefreshUnit {
  name: string;
  intervalMs: number;
  run(): Promise<void>;
}

/** Default cadence per refresh unit; callers can re-register with their own interval. */
const DEFAULT_INTERVAL_MS = 15 * 60 * 1000; // 15 minutes — fresh enough for feed data, kind to upstreams

export function buildRefreshers(deps: RefreshDeps): RefreshUnit[] {
  return Object.entries(deps.pull).map(([name, pull]) => ({
    name,
    intervalMs: DEFAULT_INTERVAL_MS,
    async run(): Promise<void> {
      await pull(); // a throw propagates — freshness must NOT advance on a failed pull
      deps.freshness.markFresh(name);
    },
  }));
}
