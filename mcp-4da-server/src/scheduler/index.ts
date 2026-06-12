/**
 * Scheduler composition (S4) — wire core + freshness + refreshers into one switchable unit.
 *
 * The whole scheduler is OFF unless explicitly enabled (fail-safe: an MCP server must never
 * grow background network activity by surprise). Enabled, it registers every refresh unit on
 * the cadence engine, starts the timers, and exposes a status view the server (or a future
 * `scheduler_status` tool) can render: every unit, its cadence, and when it last ran.
 */

import { createScheduler, type SchedulerJobView } from "./core.js";
import { createFreshness } from "./freshness.js";
import { buildRefreshers } from "./refreshers.js";

export interface InitSchedulerOpts {
  enabled: boolean;
  pull: Record<string, () => Promise<void>>;
  freshnessPath?: string;
}

export interface RunningScheduler {
  started: boolean;
  status(): SchedulerJobView[];
  stop(): void;
}

export function initScheduler(opts: InitSchedulerOpts): RunningScheduler {
  if (!opts.enabled) {
    return { started: false, status: () => [], stop: () => {} };
  }

  const freshness = createFreshness(opts.freshnessPath);
  freshness.load();
  const scheduler = createScheduler();
  for (const unit of buildRefreshers({ pull: opts.pull, freshness })) {
    scheduler.register(unit.name, unit.intervalMs, () => unit.run());
  }
  scheduler.start();

  return {
    started: true,
    status: () => scheduler.jobs(),
    stop: () => {
      scheduler.stop();
      freshness.save();
    },
  };
}
