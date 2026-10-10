// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { SystemHealthDot, collectHealthIssues } from './SystemHealthDot';

// Mock the cmd function. The component calls TWO commands:
// get_startup_health (one-shot, re-read when Settings closes) and
// get_capability_states (polled). Dispatch by command name so call ordering
// between the effects does not matter.
const mockCmd = vi.fn();
vi.mock('../lib/commands', () => ({
  cmd: (...args: unknown[]) => mockCmd(...args),
}));

// Reset store between tests so cached startupHealthIssues doesn't leak
import { useAppStore } from '../store';

type StartupIssue = { severity: 'warning' | 'error'; component: string; message: string };
type CapStates = Record<string, { state: string; reason?: string }>;

/** Route the two backend commands by name; default to clean signals. */
function wireCmd(opts: { startup?: StartupIssue[] | Error; caps?: CapStates } = {}) {
  mockCmd.mockImplementation((name: string) => {
    if (name === 'get_startup_health') {
      if (opts.startup instanceof Error) return Promise.reject(opts.startup);
      return Promise.resolve(opts.startup ?? []);
    }
    if (name === 'get_capability_states') {
      return Promise.resolve(opts.caps ?? {});
    }
    return Promise.resolve(undefined);
  });
}

const PROJECT_CONTEXT: CapStates = {
  ace_context: { state: 'degraded', reason: 'No project directories configured' },
};

describe('SystemHealthDot', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({
      startupHealthIssues: null,
      capabilityStates: null,
      showSettings: false,
      settingsInitialTab: null,
    });
  });

  it('renders nothing when health check returns no issues', async () => {
    wireCmd({ startup: [] });
    const { container } = render(<SystemHealthDot />);
    await waitFor(() => {
      expect(mockCmd).toHaveBeenCalledWith('get_startup_health');
    });
    expect(container.querySelector('button')).not.toBeInTheDocument();
  });

  it('renders nothing when health check fails', async () => {
    wireCmd({ startup: new Error('Check failed') });
    const { container } = render(<SystemHealthDot />);
    await waitFor(() => {
      expect(mockCmd).toHaveBeenCalledWith('get_startup_health');
    });
    expect(container.querySelector('button')).not.toBeInTheDocument();
  });

  it('renders a warning dot when issues are warnings only', async () => {
    wireCmd({ startup: [{ severity: 'warning', component: 'embedding', message: 'Degraded' }] });
    render(<SystemHealthDot />);
    const dot = await screen.findByRole('button');
    expect(dot.title).toBe('health.dot.warning');
    expect(dot).toHaveAttribute('data-severity', 'warning');
  });

  it('renders an error dot when errors exist', async () => {
    wireCmd({ startup: [{ severity: 'error', component: 'database', message: 'DB locked' }] });
    render(<SystemHealthDot />);
    const dot = await screen.findByRole('button');
    expect(dot.title).toBe('health.dot.error');
    expect(dot).toHaveAttribute('data-severity', 'error');
  });

  it('counts every issue', async () => {
    wireCmd({ startup: [
      { severity: 'warning', component: 'embedding', message: 'Issue 1' },
      { severity: 'warning', component: 'settings', message: 'Issue 2' },
    ] });
    render(<SystemHealthDot />);
    expect(await screen.findByRole('button')).toHaveAttribute('data-count', '2');
  });

  it('has accessible label matching the title', async () => {
    wireCmd({ startup: [{ severity: 'error', component: 'database', message: 'DB error' }] });
    render(<SystemHealthDot />);
    await waitFor(() => {
      const button = screen.getByRole('button');
      expect(button.getAttribute('aria-label')).toBe(button.title);
    });
  });

  // --- Runtime capability merge (the F-20 fix) ---

  it('renders a RED dot when a capability is unavailable, even with clean startup', async () => {
    wireCmd({
      startup: [],
      caps: { briefing_generation: { state: 'unavailable', reason: 'Anthropic rejected the API key (HTTP 401)' } },
    });
    render(<SystemHealthDot />);
    expect(await screen.findByRole('button')).toHaveAttribute('data-severity', 'error');
  });

  it('renders an AMBER dot when a capability is only degraded', async () => {
    wireCmd({
      startup: [],
      caps: { embedding_search: { state: 'degraded', reason: 'Ollama not reachable' } },
    });
    render(<SystemHealthDot />);
    expect(await screen.findByRole('button')).toHaveAttribute('data-severity', 'warning');
  });

  it('stays hidden when all capabilities are full or off by choice', async () => {
    wireCmd({
      startup: [],
      caps: {
        briefing_generation: { state: 'full' },
        embedding_search: { state: 'full' },
        llm_reranking: { state: 'off', reason: 'No AI provider chosen' },
      },
    });
    const { container } = render(<SystemHealthDot />);
    await waitFor(() => {
      expect(mockCmd).toHaveBeenCalledWith('get_capability_states');
    });
    expect(container.querySelector('button')).not.toBeInTheDocument();
  });

  // --- Fresh-profile E2E 2026-10-10 ---

  it('is a still dot: no idle animation', async () => {
    wireCmd({ startup: [], caps: PROJECT_CONTEXT });
    render(<SystemHealthDot />);
    expect((await screen.findByRole('button')).className).not.toMatch(/animate-/);
  });

  it('opens the Settings tab that fixes the warning (Projects for Project Context)', async () => {
    wireCmd({ startup: [], caps: PROJECT_CONTEXT });
    render(<SystemHealthDot />);
    fireEvent.click(await screen.findByRole('button'));
    expect(useAppStore.getState().showSettings).toBe(true);
    expect(useAppStore.getState().settingsInitialTab).toBe('projects');
  });

  it('re-checks when Settings closes, so a fixed warning clears', async () => {
    wireCmd({ startup: [], caps: PROJECT_CONTEXT });
    render(<SystemHealthDot />);
    await screen.findByRole('button');

    act(() => { useAppStore.setState({ showSettings: true }); });
    wireCmd({ startup: [], caps: { ace_context: { state: 'full' } } });
    act(() => { useAppStore.setState({ showSettings: false }); });

    await waitFor(() => {
      expect(screen.queryByRole('button')).not.toBeInTheDocument();
    });
  });
});

describe('collectHealthIssues', () => {
  it('puts errors first and maps each issue to the tab that fixes it', () => {
    const issues = collectHealthIssues(
      [{ component: 'disk', severity: 'warning', message: 'low disk' }],
      {
        ace_context: { state: 'degraded', reason: 'No project directories configured' },
        llm_reranking: { state: 'unavailable', reason: 'HTTP 401' },
        system_tray: { state: 'full' },
      },
    );
    expect(issues.map(i => [i.severity, i.fixTab])).toEqual([
      ['error', 'intelligence'],
      ['warning', 'about'],
      ['warning', 'projects'],
    ]);
  });
});
