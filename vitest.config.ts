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
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    exclude: ['node_modules', 'dist', 'src-tauri'],
    pool: 'forks',
    poolOptions: {
      forks: {
        maxForks: 1,
        memoryLimit: '512MB',
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
