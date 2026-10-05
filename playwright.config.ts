import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright E2E configuration for 4DA Tauri desktop app.
 *
 * Runs the frontend in a plain browser against a mocked Tauri IPC layer
 * (e2e/support/app.ts). Playwright starts its own Vite for this tree on
 * E2E_PORT (default 4446) — never the dev server on 4444.
 *
 * Install browsers once with: npx playwright install chromium
 */
const E2E_PORT = Number(process.env.E2E_PORT) || 4446;
export default defineConfig({
  testDir: './e2e',
  outputDir: './e2e-results',

  /* Fail fast in CI, allow retries locally */
  retries: process.env.CI ? 0 : 1,

  /* Reasonable timeouts for a local desktop app */
  timeout: 60_000,
  expect: {
    timeout: 10_000,
  },

  /* Reporter: list for terminal, HTML for detailed review */
  reporter: process.env.CI
    ? [['list'], ['html', { open: 'never', outputFolder: 'e2e-report' }]]
    : [['list']],

  use: {
    baseURL: `http://localhost:${E2E_PORT}`,

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

  /* Always a fresh Vite for THIS tree, on its own port. The suite used to
     reuse whatever served :4444 — on a dev machine that is the operator's dev
     server from another tree, so E2E silently tested someone else's code — and
     `pnpm run dev` runs kill-fourda + kill-port first. Plain vite touches
     nothing else. */
  webServer: {
    command: `npx vite --port ${E2E_PORT} --strictPort`,
    url: `http://localhost:${E2E_PORT}`,
    reuseExistingServer: false,
    // A cold start pre-bundles dependencies; 30 s was too tight on CI runners.
    timeout: 120_000,
  },
});
