import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  define: {
    __APP_VERSION__: JSON.stringify('0.0.0-test'),
  },
  test: {
    globals: true,
    environment: 'jsdom',
    // The self-hosted CI runner shares a machine with the dev fleet; under CPU
    // starvation the 5s default times out tests that pass in ~10ms isolated
    // (SprintPhase #322, then SettingsModal.keyboard + 1 more on #335 — a
    // class, not individual bad tests). 30s still catches real hangs. Local
    // default stays 5s for fast feedback.
    testTimeout: process.env.CI ? 30_000 : 5_000,
    hookTimeout: process.env.CI ? 30_000 : 10_000,
    // Explicit, because this value is load-bearing in two places and the
    // undeclared 10s default hid that. Vitest feeds `teardownTimeout` to
    // tinypool as `terminateTimeout` (the per-worker termination deadline)
    // AND uses it to arm the force-`process.exit()` watchdog in
    // `Vitest.exit()`. On the starved self-hosted runner 10s is tight enough
    // that a legitimately slow teardown reports "Failed to terminate worker";
    // 30s matches the testTimeout rationale above.
    //
    // It is NOT a hang bound. That watchdog is armed inside `exit()`, i.e.
    // only once the run has already finished, so a wedge DURING the run never
    // reaches it. The real bound is external — see the Frontend job's
    // `scripts/run-with-timeout.cjs` wrapper in .github/workflows/validate.yml.
    teardownTimeout: process.env.CI ? 30_000 : 10_000,
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    exclude: ['node_modules', 'dist', 'src-tauri'],
    pool: 'forks',
    poolOptions: {
      forks: {
        // 120 test files, each isolated in its own fork. At `maxForks: 1` that
        // is 120 jsdom environments built and torn down ONE AT A TIME, and it
        // dominates the run. Measured on the hosted Windows runner (PR #602,
        // 2026-09-01, all 120 files green):
        //
        //   Duration 170.84s
        //     environment 102.08s | prepare 10.68s | setup 11.35s
        //     collect     11.57s  | transform 2.13s | tests 13.67s
        //
        // 73% of the wall clock is per-file fork + environment overhead. The
        // tests themselves are 13.67s. That overhead is embarrassingly
        // parallel, and serialising it bought nothing.
        //
        // `maxForks: 1` was never a considered value. It arrived in c9c53442 as
        // half of a "memory limits" edit bundled into an unrelated scoring
        // commit, paired with `memoryLimit: '512MB'` — which #585 later proved
        // vitest silently ignores for this pool (see the note below). The prior
        // value was 2, set deliberately in d271809f to fix an OOM, and it held
        // for months.
        //
        // Raising it is memory-safe BECAUSE `isolate` is true (the default for
        // this pool): a fork is recycled per test file, so peak usage is bounded
        // by the heaviest single file, not by the suite. The original OOM came
        // from the unbounded default (= CPU count) on a many-core dev box —
        // which is why an explicit bound still exists at all.
        //
        // 4 is not a guess. All three values were run on the hosted runner
        // against the full 120-file suite (PR #603), green every time:
        //
        //   maxForks 1 -> 170.84s          (environment 102.08s summed)
        //   maxForks 2 -> 124.77s  (-27%)  (environment 149.06s summed)
        //   maxForks 4 -> 108.33s  (-37%)  (environment 256.08s summed)
        //
        // Diminishing returns are exactly what saturating 4 vCPU looks like:
        // 1->2 buys 46s, 2->4 buys a further 16s. Note that the `environment`
        // figure RISES as forks are added — vitest sums it across workers, so a
        // growing summed number beside a falling wall clock IS the parallelism.
        // Do not read it as a regression.
        //
        // CI: GitHub's standard hosted runner is 4 vCPU / 16 GB and ephemeral,
        // so use it. Local: 2, because that machine is also running 4DA, a Vite
        // dev server and the fleet's cargo churn — taking every core is rude and
        // is what starved these tests into timing out in the first place.
        maxForks: process.env.CI ? 4 : 2,
        // NO `memoryLimit` here. It was set to '512MB' and read as a safety
        // net during the 2026-08-31 runner-wedge investigation; it never was
        // one. Verified against the pinned vitest 3.2.6 in node_modules:
        // `createForksPool` builds its Tinypool options from maxForks,
        // minForks, isolate, execArgv and teardownTimeout only, and
        // `getWorkerMemoryLimit` reads `poolOptions.vmForks` /
        // `poolOptions.vmThreads` — never `poolOptions.forks`. The key was
        // accepted and silently ignored. Re-adding it buys nothing; if a
        // worker memory ceiling is genuinely wanted, the pool has to change
        // to `vmForks` and that is an ADR, not a one-line config edit.
      },
    },
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      exclude: [
        'node_modules/',
        'src/test/',
        '**/*.d.ts',
        '**/*.config.*',
        '**/types.ts',
      ],
      thresholds: {
        statements: 40,
        branches: 25,
        functions: 35,
        lines: 40,
      },
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
});
