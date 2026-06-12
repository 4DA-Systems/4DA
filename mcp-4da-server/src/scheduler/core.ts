/**
 * Scheduler core — the cadence engine (S1).
 *
 * A minimal, dependency-free job scheduler: register named jobs with an interval, tick them
 * manually (tests, one-shots) or let start()/stop() drive them on timers. The one hard rule:
 * the SAME job never overlaps itself — a tick that arrives while the job's fn is still
 * running resolves as a no-op instead of double-running the pull.
 */

type JobFn = () => Promise<void>;

interface Job {
  name: string;
  intervalMs: number;
  fn: JobFn;
  lastRun: number | null;
  running: boolean;
  timer: ReturnType<typeof setInterval> | null;
}

export interface SchedulerJobView {
  name: string;
  intervalMs: number;
  lastRun: number | null;
}

export interface Scheduler {
  register(name: string, intervalMs: number, fn: JobFn): void;
  start(): void;
  stop(): void;
  tick(name: string): Promise<void>;
  jobs(): SchedulerJobView[];
}

export function createScheduler(): Scheduler {
  const jobs = new Map<string, Job>();

  async function tick(name: string): Promise<void> {
    const job = jobs.get(name);
    if (!job) {
      throw new Error(`scheduler: no job registered under "${name}"`);
    }
    if (job.running) {
      return; // the same job never overlaps — a concurrent tick is a no-op
    }
    job.running = true;
    try {
      await job.fn();
      job.lastRun = Date.now();
    } finally {
      job.running = false;
    }
  }

  return {
    register(name: string, intervalMs: number, fn: JobFn): void {
      jobs.set(name, { name, intervalMs, fn, lastRun: null, running: false, timer: null });
    },

    start(): void {
      for (const job of jobs.values()) {
        if (job.timer) continue;
        job.timer = setInterval(() => {
          void tick(job.name).catch(() => {
            // a failing scheduled run must never take the process down; the job's
            // freshness simply doesn't advance, which is the observable signal
          });
        }, job.intervalMs);
      }
    },

    stop(): void {
      for (const job of jobs.values()) {
        if (job.timer) {
          clearInterval(job.timer);
          job.timer = null;
        }
      }
    },

    tick,

    jobs(): SchedulerJobView[] {
      return [...jobs.values()].map(({ name, intervalMs, lastRun }) => ({ name, intervalMs, lastRun }));
    },
  };
}
