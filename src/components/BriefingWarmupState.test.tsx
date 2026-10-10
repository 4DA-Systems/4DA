// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { useAppStore } from '../store';
import { BriefingWarmupState } from './BriefingWarmupState';

const cmdMock = vi.fn((..._a: unknown[]): Promise<unknown> => Promise.resolve([]));
vi.mock('../lib/commands', () => ({ cmd: (...a: unknown[]) => cmdMock(...(a as [string, unknown])) }));
vi.mock('../lib/startup-runtime', () => ({ isVictauriDogfoodMode: () => Promise.resolve(true) }));

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

  it('keeps the warm-up view when a stack was detected', async () => {
    setStore(true, ['rust']);
    render(<BriefingWarmupState onAnalyze={() => {}} />);
    await waitFor(() => expect(screen.getByText('OSV.dev')).toBeInTheDocument());
    expect(screen.queryByTestId('cold-start-card')).not.toBeInTheDocument();
  });
});
