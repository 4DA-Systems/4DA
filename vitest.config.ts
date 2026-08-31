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
        maxForks: 1,
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
