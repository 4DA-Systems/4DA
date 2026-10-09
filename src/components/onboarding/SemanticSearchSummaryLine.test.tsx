// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Fresh-profile E2E 2026-10-09: the onboarding summary said "Private semantic
// search active ✓" while the model was still downloading. The line now says
// what the engine's status says.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

import { SemanticSearchSummaryLine, semanticSummary, type EngineStatus } from './SemanticSearchSummaryLine';

const status = (state: EngineStatus['state'], done = 0, total = 0): EngineStatus => ({
  state,
  message: null,
  bytes_downloaded: done,
  bytes_total: total,
});

describe('semanticSummary', () => {
  it('says active only when semantic search actually works', () => {
    expect(semanticSummary(status('ready'), null).key).toBe('calibration.onboarding.summaryAI');
    expect(semanticSummary(status('preparing'), 'semantic').key).toBe('calibration.onboarding.summaryAI');
    expect(semanticSummary(status('preparing'), null).key).not.toBe('calibration.onboarding.summaryAI');
    expect(semanticSummary(null, null).key).not.toBe('calibration.onboarding.summaryAI');
    expect(semanticSummary(status('idle'), null).key).not.toBe('calibration.onboarding.summaryAI');
  });

  it('shows download progress in MB while the model downloads', () => {
    const s = semanticSummary(status('preparing', 100 * 1_048_576, 274 * 1_048_576), null);
    expect(s).toEqual({
      tone: 'pending',
      key: 'calibration.onboarding.summaryDownloading',
      params: { done: 100, total: 274 },
    });
  });

  it('reports a failed engine as a warning, and keyword-only as before', () => {
    expect(semanticSummary(status('failed'), null)).toMatchObject({ tone: 'warn', key: 'calibration.onboarding.summaryFailed' });
    expect(semanticSummary(status('ready'), 'keyword-only').key).toBe('calibration.onboarding.summaryKeyword');
  });
});

describe('SemanticSearchSummaryLine', () => {
  beforeEach(() => {
    cmdMock.mockReset();
  });

  it('renders the live engine status, not an optimistic check mark', async () => {
    cmdMock.mockResolvedValue(status('preparing', 5 * 1_048_576, 274 * 1_048_576));
    render(<SemanticSearchSummaryLine embeddingMode={null} />);
    await act(async () => {});
    expect(cmdMock).toHaveBeenCalledWith('get_embedding_engine_status');
    expect(screen.getByTestId('semantic-search-summary')).toHaveTextContent('calibration.onboarding.summaryDownloading');
    expect(screen.queryByText('calibration.onboarding.summaryAI')).not.toBeInTheDocument();
  });

  it('a failed status call never shows "active"', async () => {
    cmdMock.mockRejectedValue(new Error('ipc down'));
    render(<SemanticSearchSummaryLine embeddingMode={null} />);
    await act(async () => {});
    expect(screen.getByTestId('semantic-search-summary')).toHaveTextContent('calibration.onboarding.summaryStarting');
  });
});
