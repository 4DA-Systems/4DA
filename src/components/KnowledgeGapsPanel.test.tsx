// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Fresh-profile E2E 2026-10-09: a user who skipped the project scan was told
// "No gaps detected — your knowledge is current". With no dependency known,
// nothing was checked; the panel now offers the scan instead.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

let coldStart = false;
vi.mock('../hooks/use-cold-start-gate', () => ({
  useColdStartGate: () => coldStart,
}));

vi.mock('./ProGate', () => ({
  ProGate: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('./ContentTranslationProvider', () => ({
  useTranslatedContent: () => ({ getTranslated: (_id: string, text: string) => text }),
}));

const runAutoDiscovery = vi.fn(() => Promise.resolve());
const loadUserContext = vi.fn(() => Promise.resolve());
const startAnalysis = vi.fn(() => Promise.resolve());
vi.mock('../store', () => ({
  useAppStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ isScanning: false, runAutoDiscovery, loadUserContext, startAnalysis }),
}));

import { KnowledgeGapsPanel } from './KnowledgeGapsPanel';

describe('KnowledgeGapsPanel', () => {
  beforeEach(() => {
    cmdMock.mockReset();
    runAutoDiscovery.mockClear();
    coldStart = false;
  });

  it('never claims "current" when no dependency is known, and offers the scan', async () => {
    cmdMock.mockResolvedValue({ items: [], total: 0, total_tracked: 0 });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});

    expect(screen.getByTestId('knowledge-gaps-scan-prompt')).toHaveTextContent('knowledgeGaps.noDependencies');
    expect(screen.queryByText(/your knowledge is current/)).not.toBeInTheDocument();
    expect(screen.queryByText('knowledgeGaps.noGaps')).not.toBeInTheDocument();

    await act(async () => {
      fireEvent.click(screen.getByText('onboarding.choice.scanProjects'));
    });
    expect(runAutoDiscovery).toHaveBeenCalledTimes(1);
  });

  it('offers the scan on day one too: no lockfile is not a cold-start silence', async () => {
    coldStart = true;
    cmdMock.mockResolvedValue({ items: [], total: 0, total_tracked: 0 });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.getByTestId('knowledge-gaps-scan-prompt')).toBeInTheDocument();
  });

  it('keeps the all-clear for a user whose dependencies were checked', async () => {
    cmdMock.mockResolvedValue({ items: [], total: 0, total_tracked: 94 });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.queryByTestId('knowledge-gaps-scan-prompt')).not.toBeInTheDocument();
    expect(screen.getByText('knowledgeGaps.title')).toBeInTheDocument();
  });
});
