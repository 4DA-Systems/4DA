// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SystemHealthDot } from './SystemHealthDot';

// Mock the cmd function. The component now calls TWO commands:
// get_startup_health (one-shot) and get_capability_states (polled). Dispatch
// by command name so call ordering between the two effects does not matter.
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

describe('SystemHealthDot', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({ startupHealthIssues: null, capabilityStates: null });
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
    await waitFor(() => {
      expect(screen.getByRole('button')).toBeInTheDocument();
    });
    expect(screen.getByRole('button').title).toContain('warning');
  });

  it('renders an error dot when errors exist', async () => {
    wireCmd({ startup: [{ severity: 'error', component: 'database', message: 'DB locked' }] });
    render(<SystemHealthDot />);
    await waitFor(() => {
      expect(screen.getByRole('button')).toBeInTheDocument();
    });
    expect(screen.getByRole('button').title).toContain('error');
  });

  it('shows issue count in title', async () => {
    wireCmd({ startup: [
      { severity: 'warning', component: 'embedding', message: 'Issue 1' },
      { severity: 'warning', component: 'settings', message: 'Issue 2' },
    ] });
    render(<SystemHealthDot />);
    await waitFor(() => {
      expect(screen.getByRole('button')).toBeInTheDocument();
    });
    expect(screen.getByRole('button').title).toContain('2');
  });

  it('calls onClick when clicked', async () => {
    wireCmd({ startup: [{ severity: 'warning', component: 'sources', message: 'No sources' }] });
    const onClick = vi.fn();
    render(<SystemHealthDot onClick={onClick} />);
    await waitFor(() => {
      expect(screen.getByRole('button')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalledTimes(1);
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
    await waitFor(() => {
      expect(screen.getByRole('button')).toBeInTheDocument();
    });
    expect(screen.getByRole('button').title).toContain('error');
  });

  it('renders an AMBER dot when a capability is only degraded', async () => {
    wireCmd({
      startup: [],
      caps: { embedding_search: { state: 'degraded', reason: 'Ollama not reachable' } },
    });
    render(<SystemHealthDot />);
    await waitFor(() => {
      expect(screen.getByRole('button')).toBeInTheDocument();
    });
    expect(screen.getByRole('button').title).toContain('warning');
  });

  it('stays hidden when all capabilities are full and startup is clean', async () => {
    wireCmd({
      startup: [],
      caps: { briefing_generation: { state: 'full' }, embedding_search: { state: 'full' } },
    });
    const { container } = render(<SystemHealthDot />);
    await waitFor(() => {
      expect(mockCmd).toHaveBeenCalledWith('get_capability_states');
    });
    expect(container.querySelector('button')).not.toBeInTheDocument();
  });
});
