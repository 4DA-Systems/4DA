// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { Locator, Page } from '@playwright/test';

/**
 * Shared helpers for the browser-mode E2E suite (Vite dev server, no Tauri
 * backend). Every IPC call either goes through `installTauriIpcMock` or fails,
 * which is exactly the "backend errored" path the app must survive.
 */

/** The three main views (AD-054), by their `tab-<id>` id and rendered English label (ViewTabBar TABS). */
export const MAIN_TABS = [
  { id: 'briefing', label: 'Brief' },
  { id: 'preemption', label: 'Preemption' },
  { id: 'results', label: 'Signal' },
] as const;

/** Preemption's sub-views, by their `preemption-tab-<id>` id and English label. */
export const PREEMPTION_SUB_VIEWS = [
  { id: 'worklist', label: 'Worklist' },
  { id: 'blindspots', label: 'Blind Spots' },
  { id: 'knowledge', label: 'Learning Opportunities' },
] as const;

export function mainTablist(page: Page): Locator {
  return page.getByRole('tablist', { name: /content views/i });
}

export function preemptionTablist(page: Page): Locator {
  return page.getByRole('tablist', { name: 'Preemption views' });
}

/**
 * Load the app and wait until the splash has lifted and the main shell is
 * interactive. Checking right after `goto` races the splash, and an
 * `isVisible()` that resolves false there silently skips the whole test.
 */
export async function gotoMainShell(page: Page): Promise<void> {
  await page.goto('/');
  await mainTablist(page).waitFor({ state: 'visible', timeout: 20_000 });
}

/** The app bar (UnifiedAppBar): a banner landmark named after the app. */
export function appBar(page: Page): Locator {
  return page.getByRole('banner', { name: '4DA' });
}

/**
 * React error-boundary fallbacks — a component tree that crashed, as opposed
 * to a view that caught its own IPC failure and rendered a designed error
 * state (e.g. Blind Spots' "Coverage scan unavailable … Retry").
 *  - ErrorBoundary (app):      <h1>Something went wrong</h1>
 *  - ViewErrorBoundary (view): <h2>{view} failed to load</h2>
 *  - PanelErrorBoundary:       <p>{panel} failed to load.</p>
 */
export function crashFallbacks(page: Page): Locator {
  return page
    .getByRole('heading', { name: /^something went wrong$|failed to load$/i })
    .or(page.getByText(/^.+ failed to load\.$/));
}

export interface TauriMockOptions {
  /** Payload for `get_settings`. */
  settings: Record<string, unknown>;
  /** Extra command handlers: a value to resolve with. */
  responses?: Record<string, unknown>;
}

/**
 * Install a minimal `__TAURI_INTERNALS__` so the app runs its desktop code
 * paths. Every IPC call is recorded in `window.__ipc_log`. Commands with no
 * response configured REJECT, like a backend error — answering `null` instead
 * hands components a value the real command can never return (a `Vec` never
 * serializes as null) and crashes them in ways the desktop app cannot.
 */
export async function installTauriIpcMock(page: Page, options: TauriMockOptions): Promise<void> {
  const responses: Record<string, unknown> = {
    get_settings: options.settings,
    get_onboarding_status: { completed: true, skipped: false },
    detect_environment: {
      has_anthropic_env: false, anthropic_env_preview: '', has_openai_env: false, openai_env_preview: '',
    },
    get_ollama_status: { running: false, version: '', models: [] },
    check_ollama_status: { running: false, version: '', models: [] },
    // Discovery is consent-first: listing folders reads nothing, and no
    // spec should see a scan it did not ask for.
    ace_preview_discovery_dirs: [],
    ace_candidate_dev_roots: [],
    get_trial_status: { active: false, days_remaining: 0, started_at: null },
    get_model_registry: { providers: {}, fetched_at: 0, version: '0.0.0' },
    list_projects_with_stack_status: [],
    get_standing_queries: [],
    ...options.responses,
  };
  await page.addInitScript((table: Record<string, unknown>) => {
    const w = window as unknown as Record<string, unknown> & { __ipc_log: unknown[] };
    w.__ipc_log = [];
    let nextId = 1;
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    w.__TAURI_INTERNALS__ = {
      invoke: async (cmd: string, args?: unknown) => {
        w.__ipc_log.push({ cmd, args: JSON.parse(JSON.stringify(args ?? {})) });
        if (cmd === 'plugin:event|listen') return nextId++;
        if (cmd === 'plugin:event|unlisten') return null;
        if (Object.prototype.hasOwnProperty.call(table, cmd)) return table[cmd];
        if (cmd.startsWith('set_')) return null;
        throw new Error(`[e2e mock] no backend for ${cmd}`);
      },
      transformCallback: (cb: unknown) => {
        const id = nextId++;
        w[`_${id}`] = cb;
        return id;
      },
      unregisterCallback: (id: number) => {
        delete w[`_${id}`];
      },
      convertFileSrc: (path: string) => path,
    };
  }, responses);
}

/**
 * `check_ollama_status` as the backend returns it for a machine with a chat
 * model installed (settings_commands_llm/ollama.rs): readiness flags included.
 */
export const OLLAMA_READY = {
  running: true,
  version: '0.12.0',
  models: [{ name: 'gemma4:12b', size: 7_300_000_000, modified_at: '2026-10-01T00:00:00Z' }],
  url: 'http://localhost:11434',
  has_llm_model: true,
  has_embedding_model: false,
  recommended_judge: 'gemma4:12b',
};

export interface IpcCall {
  cmd: string;
  args: Record<string, unknown>;
}

export async function ipcLog(page: Page): Promise<IpcCall[]> {
  return page.evaluate(() => (window as unknown as { __ipc_log?: IpcCall[] }).__ipc_log ?? []);
}

/**
 * A `get_settings` payload in the exact shape the backend returns
 * (settings_commands.rs `get_settings`). The old inline mocks sent a stale
 * shape with no `rerank` block: the settings slice threw applying it, kept its
 * defaults (provider "anthropic") and never reflected the mocked provider.
 */
export function mockSettings(llm: { provider: string; model: string; has_api_key?: boolean; base_url?: string | null }): Record<string, unknown> {
  return {
    llm: { has_api_key: false, base_url: null, ...llm },
    rerank: {
      enabled: false,
      max_items_per_batch: 15,
      min_embedding_score: 0.25,
      daily_token_limit: 100000,
      daily_cost_limit_cents: 50,
    },
    llm_limits: { daily_token_limit: 100000, daily_cost_limit_cents: 50 },
    usage: { tokens_today: 0, cost_today_cents: 0, tokens_total: 0, items_reranked: 0 },
    embedding_threshold: 0.5,
    onboarding_complete: true,
    auto_discovery_completed: true,
    auto_assess_blind_spots: true,
    license: { tier: 'free', has_key: false, activated_at: null },
  };
}
