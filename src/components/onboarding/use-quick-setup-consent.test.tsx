// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// First-run E2E findings (audit 2026-10-07): a ready Ollama read as "models not
// installed", Enter saved Ollama nobody picked, a StrictMode double mount turned
// the scan into "nothing found", and projects were scanned before any consent.

import { StrictMode } from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, renderHook, screen, fireEvent, act, waitFor } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));
vi.mock('../../lib/tauri-events', () => ({
  safeListen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock('./setup-ai-provider', () => ({ SetupAIProvider: () => <div /> }));
vi.mock('./setup-stack', () => ({ SetupStack: () => <div /> }));
vi.mock('./setup-interests', () => ({ SetupInterests: () => <div /> }));
vi.mock('./setup-locale', () => ({ SetupLocale: () => <div /> }));

import { resetDiscoveryForTests } from '../../lib/discovery';
import { QuickSetupStep } from './QuickSetupStep';
import { useQuickSetup } from './use-quick-setup';
import { useTrialEndDate } from './use-trial-end-date';

const tag = (name: string) => ({ name, size: 1, modified_at: '' });
const HOME = ['C:\\Users\\dev\\code', 'C:\\Users\\dev\\Documents'];
const SCANNED = { success: true, directories: [HOME[0]], scan_result: { combined: { topics: ['rust', 'react'] } } };

function installBackend(overrides: Record<string, unknown> = {}) {
  cmdMock.mockImplementation((command: string) => {
    if (command in overrides) {
      const v = overrides[command];
      return Promise.resolve(typeof v === 'function' ? (v as () => unknown)() : v);
    }
    switch (command) {
      case 'check_ollama_status': return Promise.resolve({ running: false, models: [] });
      case 'ace_preview_discovery_dirs': return Promise.resolve(HOME);
      case 'ace_auto_discover': return Promise.resolve(SCANNED);
      case 'taste_test_is_calibrated': return Promise.resolve(false);
      case 'ace_get_suggested_interests': return Promise.resolve([]);
      default: return Promise.resolve();
    }
  });
}

const props = { isAnimating: false, onComplete: vi.fn(), onBack: vi.fn() };
const calls = (name: string) => cmdMock.mock.calls.filter(c => c[0] === name);

beforeEach(() => {
  cmdMock.mockReset();
  resetDiscoveryForTests();
  try { localStorage.clear(); } catch { /* noop */ }
  installBackend();
});

describe('useQuickSetup — local AI readiness', () => {
  it('an installed chat model (gemma4:12b) is ready: no download is offered', async () => {
    installBackend({ check_ollama_status: { running: true, version: '0.12', models: [tag('gemma4:12b')], url: 'http://localhost:11434' } });
    const { result } = renderHook(() => useQuickSetup(props));
    await act(async () => {});

    expect(result.current.aiConfigured).toBe(true);
    expect(result.current.provider).toBe('ollama');
  });

  it('Enter with no explicit choice never saves Ollama', async () => {
    // Running, but only an embedder: not ready, so nothing is preselected.
    installBackend({ check_ollama_status: { running: true, models: [tag('nomic-embed-text')] } });
    const onComplete = vi.fn();
    const { result } = renderHook(() => useQuickSetup({ ...props, onComplete }));
    await act(async () => {});
    expect(result.current.provider).toBeNull();

    await act(async () => { await result.current.handleContinue(); });

    const saved = calls('set_llm_provider').map(c => (c[1] as { provider?: string }).provider);
    expect(saved).not.toContain('ollama');
    expect(onComplete).toHaveBeenCalled();
  });

  it('does not scan any project on mount', async () => {
    renderHook(() => useQuickSetup(props));
    await act(async () => {});
    expect(calls('ace_auto_discover')).toHaveLength(0);
    expect(calls('ace_preview_discovery_dirs').length).toBeGreaterThan(0);
  });
});

async function openProjects() {
  render(<StrictMode><QuickSetupStep {...props} /></StrictMode>);
  fireEvent.click(screen.getByRole('button', { name: /onboarding\.setup\.yourProjects/ }));
  return screen.findByLabelText(HOME[1]!);
}

describe('QuickSetupStep — consented project scan', () => {
  it('Scan sends only the folders the user left ticked', async () => {
    fireEvent.click(await openProjects());
    fireEvent.click(screen.getByText('onboarding.projects.scanSelected'));

    await waitFor(() => expect(calls('ace_auto_discover')).toHaveLength(1));
    expect(calls('ace_auto_discover')[0]![1]).toEqual({ dirs: [HOME[0]] });
    expect(await screen.findByText('rust')).toBeInTheDocument();
  });

  it('an "already running" answer under StrictMode is never shown as "nothing found"', async () => {
    let n = 0;
    installBackend({
      ace_auto_discover: () => (n++ === 0 ? { success: false, status: 'already_running' } : SCANNED),
    });
    await openProjects();
    fireEvent.click(screen.getByText('onboarding.projects.scanSelected'));

    await act(async () => {});
    expect(screen.queryByText('onboarding.projects.noTech')).not.toBeInTheDocument();
    expect(await screen.findByText('rust', {}, { timeout: 4000 })).toBeInTheDocument();
    expect(screen.queryByText('onboarding.projects.noTech')).not.toBeInTheDocument();
  });

  it('the stack header asks the user to pick a stack instead of claiming to detect one', async () => {
    await openProjects();
    expect(screen.getAllByText('onboarding.setup.pickStack').length).toBeGreaterThan(0);
    expect(screen.queryByText('onboarding.setup.autoDetecting')).not.toBeInTheDocument();
  });
});

describe('useTrialEndDate — trial hint follows trial state', () => {
  it('gives an end date while the trial is active', async () => {
    installBackend({ get_trial_status: { active: true, days_remaining: 14, started_at: '2026-10-08T00:00:00Z' } });
    const { result } = renderHook(() => useTrialEndDate('en'));
    await waitFor(() => expect(result.current).not.toBeNull());
  });

  it('gives nothing when no trial is running', async () => {
    installBackend({ get_trial_status: { active: false, days_remaining: 0, started_at: '2026-01-01T00:00:00Z' } });
    const { result } = renderHook(() => useTrialEndDate('en'));
    await act(async () => {});
    expect(result.current).toBeNull();
  });
});
