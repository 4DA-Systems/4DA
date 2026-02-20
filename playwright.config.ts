import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright E2E configuration for 4DA Tauri desktop app.
 *
 * The Vite dev server must be running on localhost:4444 before tests start.
 * Start it with: pnpm run dev
 *
 * Install browsers once with: npx playwright install chromium
 */
export default defineConfig({
  testDir: './e2e',
  outputDir: './e2e-results',

  /* Fail fast in CI, allow retries locally */
  retries: process.env.CI ? 0 : 1,

  /* Reasonable timeouts for a local desktop app */
  timeout: 30_000,
  expect: {
    timeout: 10_000,
  },

  /* Reporter: list for terminal, HTML for detailed review */
  reporter: process.env.CI
    ? [['list'], ['html', { open: 'never', outputFolder: 'e2e-report' }]]
    : [['list']],

  use: {
    baseURL: 'http://localhost:4444',

    /* Capture evidence on failure */
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
    video: 'retain-on-failure',

    /* Sensible defaults */
    actionTimeout: 10_000,
    navigationTimeout: 15_000,
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],

  /* Do NOT auto-start the dev server. The user manages it. */
  // webServer: { command: 'pnpm run dev', url: 'http://localhost:4444' },
});
