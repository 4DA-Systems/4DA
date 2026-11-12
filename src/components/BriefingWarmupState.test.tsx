// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act, render, screen, waitFor } from '@testing-library/react';
import { useAppStore } from '../store';
import { BriefingWarmupState } from './BriefingWarmupState';

const cmdMock = vi.fn((..._a: unknown[]): Promise<unknown> => Promise.resolve([]));
vi.mock('../lib/commands', () => ({ cmd: (...a: unknown[]) => cmdMock(...(a as [string, unknown])) }));
const dogfood = { on: true };
vi.mock('../lib/startup-runtime', () => ({ isVictauriDogfoodMode: () => Promise.resolve(dogfood.on) }));

const stackOnly = [
  { type: 'osv', name: 'OSV.dev', enabled: true, class: 'stack' },
  { type: 'hackernews', name: 'Hacker News', enabled: false, class: 'interest' },
];

function setStore(analysisComplete: boolean, techStack: string[]) {
  const s = useAppStore.getState();
  useAppStore.setState({
    appState: { ...s.appState, analysisComplete },
    userContext: { ...(s.userContext ?? {}), tech_stack: techStack } as typeof s.userContext,
  });
}

beforeEach(() => {
  cmdMock.mockReset();
  cmdMock.mockImplementation(() => Promise.resolve(stackOnly));
  dogfood.on = true;
  window.sessionStorage.clear();
  useAppStore.setState({ settingsLoaded: true, showOnboarding: false, isFirstRun: false });
});

describe('BriefingWarmupState — cold start (doctrine rule 6)', () => {
  it('shows the setup card, not an empty state, when no project and no interest exist', async () => {
    setStore(true, []);
    render(<BriefingWarmupState onAnalyze={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('cold-start-card')).toBeInTheDocument());
  });

  it('keeps the warm-up view while the first run is still going', async () => {
    setStore(false, []);
    render(<BriefingWarmupState onAnalyze={() => {}} />);
    await waitFor(() => expect(screen.getByText('OSV.dev')).toBeInTheDocument());
    expect(screen.queryByTestId('cold-start-card')).not.toBeInTheDocument();
    expect(screen.queryByText('Hacker News')).not.toBeInTheDocument();
  });

  it('offers the brief, not another analysis, once an analysis finished', async () => {
    setStore(true, ['rust']);
    render(<BriefingWarmupState onAnalyze={() => {}} />);
    await waitFor(() => expect(screen.getByText('briefing.readyToGenerate')).toBeInTheDocument());
    expect(screen.queryByText('briefing.warmup.activate')).not.toBeInTheDocument();
    expect(screen.queryByTestId('cold-start-card')).not.toBeInTheDocument();
  });
});

describe('BriefingWarmupState — auto-start (audit 2026-11-11)', () => {
  beforeEach(() => {
    dogfood.on = false;
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  async function settle(ms: number) {
    await act(async () => { await vi.advanceTimersByTimeAsync(ms); });
  }

  it('never starts an analysis behind the onboarding wizard', async () => {
    useAppStore.setState({ showOnboarding: true });
    setStore(false, []);
    const onAnalyze = vi.fn();
    render(<BriefingWarmupState onAnalyze={onAnalyze} />);
    await settle(5000);
    expect(onAnalyze).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it('never starts one before the onboarding decision is made', async () => {
    useAppStore.setState({ settingsLoaded: false, showOnboarding: false });
    setStore(false, []);
    const onAnalyze = vi.fn();
    render(<BriefingWarmupState onAnalyze={onAnalyze} />);
    await settle(5000);
    expect(onAnalyze).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it('starts one analysis per session, not one per mount', async () => {
    setStore(false, []);
    const onAnalyze = vi.fn();
    const first = render(<BriefingWarmupState onAnalyze={onAnalyze} />);
    await settle(3500);
    expect(onAnalyze).toHaveBeenCalledTimes(1);
    first.unmount();
    // A minute later the user comes back to the Brief tab (offline: still no rows).
    await settle(60_000);
    render(<BriefingWarmupState onAnalyze={onAnalyze} />);
    await settle(5000);
    expect(onAnalyze).toHaveBeenCalledTimes(1);
    vi.useRealTimers();
  });
});
