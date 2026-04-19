// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * AdvisorPanel — Intelligence Mesh Phase 7b + 7c component test.
 *
 * Validates the receipts panel shows advisor identity, normalized scores,
 * reasoning, kind-specific disagreement narrative, and the Phase 7c
 * calibration status surface (badge + refit button).
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { AdvisorPanel } from './AdvisorPanel';
import type { AdvisorSignal } from '../../types/analysis';

// Mock the typed cmd() layer — the badge calls
// `get_calibration_curve_status` on mount when identity_hash is present,
// and the Refit button calls `fit_calibration_curves_now`.
const mockCmd = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (name: string, args?: unknown) => mockCmd(name, args),
}));

// Mock Tauri event listener (used by the badge to refresh on
// calibration-curves-updated). No test exercises the live-update
// path, so a no-op unlisten is sufficient.
vi.mock('@tauri-apps/api/event', () => ({
  listen: () => Promise.resolve(() => {}),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) => {
      const table: Record<string, string> = {
        'scoreDrawer.advisors': 'Advisors',
        'scoreDrawer.split': 'Split',
        'scoreDrawer.advisorSkeptical':
          'Pipeline rated this relevant; the LLM judge was skeptical.',
        'scoreDrawer.advisorEnthusiastic':
          'Pipeline rated this lower; the LLM judge was enthusiastic.',
        'scoreDrawer.advisorsInternal':
          'Multiple advisors disagreed with each other.',
        'scoreDrawer.advisorConfidence': `confidence ${vars?.pct ?? ''}%`,
        'scoreDrawer.preMesh': 'uncalibrated',
        'scoreDrawer.preMeshTooltip': 'no calibration yet',
        'scoreDrawer.calibrated': 'calibrated',
        'scoreDrawer.calibratedTooltip': `Fit from ${vars?.samples ?? ''}`,
        'scoreDrawer.stale': 'recalibrating',
        'scoreDrawer.staleTooltip': 'curve stale',
        'scoreDrawer.refit': 'Refit now',
        'scoreDrawer.refitInProgress': 'Refitting…',
        'scoreDrawer.refitSuccess': `Fit ${vars?.count ?? ''} curve(s)`,
        'scoreDrawer.refitNoCurves': 'No curves produced',
        'scoreDrawer.refitFailed': 'Refit failed',
        'scoreDrawer.promptVersionTooltip': 'prompt version tooltip',
      };
      return table[key] ?? key;
    },
  }),
}));

function makeSignal(overrides: Partial<AdvisorSignal> = {}): AdvisorSignal {
  return {
    provider: 'ollama',
    model: 'llama3.2',
    task: 'judge',
    raw_score: 0.82,
    normalized_score: 0.82,
    confidence: 0.82,
    reason: 'Looks directly related to the user context.',
    prompt_version: 'judge-v1-2026-04-15',
    calibration_id: 'pre-mesh-unknown',
    ...overrides,
  };
}

describe('AdvisorPanel', () => {
  beforeEach(() => {
    mockCmd.mockReset();
  });

  it('renders nothing when there are no signals and no disagreement', () => {
    const { container } = render(
      <AdvisorPanel advisorSignals={undefined} disagreement={null} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders nothing when signals is an empty array and no disagreement', () => {
    const { container } = render(
      <AdvisorPanel advisorSignals={[]} disagreement={undefined} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders the Advisors heading when at least one signal exists', () => {
    render(
      <AdvisorPanel advisorSignals={[makeSignal()]} disagreement={null} />,
    );
    expect(screen.getByTestId('advisor-panel')).toBeTruthy();
    expect(screen.getByText('Advisors')).toBeTruthy();
  });

  it('renders provider/model, normalized score, confidence, and reason', () => {
    render(
      <AdvisorPanel
        advisorSignals={[
          makeSignal({
            provider: 'anthropic',
            model: 'claude-sonnet-4-6',
            normalized_score: 0.91,
            confidence: 0.88,
            reason: 'Matches the sovereign-intelligence theme directly.',
          }),
        ]}
        disagreement={null}
      />,
    );
    const row = screen.getByTestId('advisor-row');
    expect(row.getAttribute('data-provider')).toBe('anthropic');
    expect(row.getAttribute('data-model')).toBe('claude-sonnet-4-6');
    expect(screen.getByText('anthropic/claude-sonnet-4-6')).toBeTruthy();
    expect(screen.getByText('91%')).toBeTruthy();
    expect(screen.getByText(/confidence 88%/)).toBeTruthy();
    expect(
      screen.getByText(/sovereign-intelligence theme directly/),
    ).toBeTruthy();
  });

  it('shows the uncalibrated tag for pre-mesh advisors', () => {
    render(
      <AdvisorPanel
        advisorSignals={[makeSignal({ calibration_id: 'pre-mesh-unknown' })]}
        disagreement={null}
      />,
    );
    expect(screen.getByText('uncalibrated')).toBeTruthy();
  });

  it('shows uncalibrated tag when identity_hash is missing (legacy signal)', () => {
    // A signal carrying a real calibration_id but no identity_hash
    // can't be looked up — fall back to uncalibrated rather than
    // show stale placeholder data. Lookup is NOT attempted.
    render(
      <AdvisorPanel
        advisorSignals={[
          makeSignal({
            calibration_id: 'judge-llama3.2-cal-v1-2026-05-01',
            identity_hash: null,
          }),
        ]}
        disagreement={null}
      />,
    );
    expect(screen.getByText('uncalibrated')).toBeTruthy();
    expect(mockCmd).not.toHaveBeenCalledWith(
      'get_calibration_curve_status',
      expect.anything(),
    );
  });

  it('fetches and displays calibrated badge when a real curve exists', async () => {
    mockCmd.mockImplementation((name: string) => {
      if (name === 'get_calibration_curve_status') {
        return Promise.resolve({
          curve_id: 'judge-abc12345-cal-v1-2026-04-16',
          task: 'judge',
          prompt_version: 'judge-v1-2026-04-15',
          brier_score: 0.14,
          ece: 0.08,
          sample_count: 120,
          created_at: '2026-04-14T10:00:00Z',
          age_days: 2,
          is_stale: false,
        });
      }
      return Promise.resolve();
    });

    render(
      <AdvisorPanel
        advisorSignals={[
          makeSignal({
            identity_hash: 'abc12345def67890',
            calibration_id: 'judge-abc12345-cal-v1-2026-04-16',
          }),
        ]}
        disagreement={null}
      />,
    );

    await waitFor(() =>
      expect(screen.getByText('calibrated')).toBeTruthy(),
    );
    expect(mockCmd).toHaveBeenCalledWith(
      'get_calibration_curve_status',
      expect.objectContaining({
        identityHash: 'abc12345def67890',
        task: 'judge',
        currentPromptVersion: 'judge-v1-2026-04-15',
      }),
    );
  });

  it('shows recalibrating badge when the fetched curve is stale', async () => {
    mockCmd.mockImplementation(() =>
      Promise.resolve({
        curve_id: 'judge-abc12345-cal-v1-2026-04-10',
        task: 'judge',
        prompt_version: 'judge-v1-old',
        brier_score: 0.2,
        ece: 0.15,
        sample_count: 80,
        created_at: '2026-04-10T10:00:00Z',
        age_days: 6,
        is_stale: true,
      }),
    );

    render(
      <AdvisorPanel
        advisorSignals={[
          makeSignal({
            identity_hash: 'abc12345',
            calibration_id: 'judge-abc12345-cal-v1-2026-04-10',
          }),
        ]}
        disagreement={null}
      />,
    );

    await waitFor(() =>
      expect(screen.getByText('recalibrating')).toBeTruthy(),
    );
  });

  it('renders the skeptical disagreement narrative', () => {
    render(
      <AdvisorPanel
        advisorSignals={[makeSignal()]}
        disagreement="AdvisorSkeptical"
      />,
    );
    const disagreement = screen.getByTestId('advisor-panel-disagreement');
    expect(disagreement.textContent).toMatch(/skeptical/i);
  });

  it('renders the enthusiastic disagreement narrative', () => {
    render(
      <AdvisorPanel
        advisorSignals={[makeSignal()]}
        disagreement="AdvisorEnthusiastic"
      />,
    );
    expect(screen.getByTestId('advisor-panel-disagreement').textContent).toMatch(
      /enthusiastic/i,
    );
  });

  it('renders the internal-disagreement narrative', () => {
    render(
      <AdvisorPanel
        advisorSignals={[makeSignal(), makeSignal({ model: 'qwen2.5' })]}
        disagreement="AdvisorsInternal"
      />,
    );
    expect(screen.getByTestId('advisor-panel-disagreement').textContent).toMatch(
      /disagreed with each other/i,
    );
  });

  it('renders the panel even when signals is empty but disagreement is set', () => {
    // Edge case: the pipeline might flag a disagreement while the
    // rerank stage didn't persist per-advisor signals (legacy-path
    // items re-scored later). Panel still renders the explanation.
    render(<AdvisorPanel advisorSignals={[]} disagreement="AdvisorSkeptical" />);
    expect(screen.getByTestId('advisor-panel')).toBeTruthy();
    expect(screen.getByTestId('advisor-panel-disagreement')).toBeTruthy();
    expect(screen.queryByTestId('advisor-row')).toBeNull();
  });

  // ── Refit button (Phase 7c) ────────────────────────────────────────

  it('shows the Refit button whenever at least one advisor signal exists', () => {
    render(
      <AdvisorPanel advisorSignals={[makeSignal()]} disagreement={null} />,
    );
    expect(screen.getByTestId('refit-button')).toBeTruthy();
  });

  it('does not show the Refit button when there are no signals', () => {
    render(
      <AdvisorPanel advisorSignals={[]} disagreement="AdvisorSkeptical" />,
    );
    expect(screen.queryByTestId('refit-button')).toBeNull();
  });

  it('renders the success state after a fit produces curves', async () => {
    mockCmd.mockImplementation((name: string) => {
      if (name === 'fit_calibration_curves_now') {
        return Promise.resolve({
          total_candidates: 2,
          curves_produced: 2,
          fits: [],
        });
      }
      return Promise.resolve();
    });

    render(
      <AdvisorPanel advisorSignals={[makeSignal()]} disagreement={null} />,
    );
    fireEvent.click(screen.getByTestId('refit-button'));
    await waitFor(() => {
      const status = screen.getByTestId('refit-status');
      expect(status.getAttribute('data-refit-state')).toBe('success');
      expect(status.textContent).toMatch(/2 curve/);
    });
  });

  it('renders the empty state when the fitter produced no curves', async () => {
    mockCmd.mockImplementation(() =>
      Promise.resolve({
        total_candidates: 1,
        curves_produced: 0,
        fits: [
          {
            model_identity_hash: 'hash',
            provider: 'ollama',
            model: 'llama3.2',
            task: 'judge',
            samples_scanned: 5,
            samples_labeled: 3,
            curve_saved: false,
            curve_id: null,
            brier_score: null,
            ece: null,
            skipped_reason: 'only 3 labeled samples (need >= 50)',
          },
        ],
      }),
    );

    render(
      <AdvisorPanel advisorSignals={[makeSignal()]} disagreement={null} />,
    );
    fireEvent.click(screen.getByTestId('refit-button'));
    await waitFor(() => {
      const status = screen.getByTestId('refit-status');
      expect(status.getAttribute('data-refit-state')).toBe('empty');
    });
  });

  it('renders the error state when the fit command throws', async () => {
    mockCmd.mockImplementation(() =>
      Promise.reject(new Error('DB locked')),
    );
    render(
      <AdvisorPanel advisorSignals={[makeSignal()]} disagreement={null} />,
    );
    fireEvent.click(screen.getByTestId('refit-button'));
    await waitFor(() => {
      const status = screen.getByTestId('refit-status');
      expect(status.getAttribute('data-refit-state')).toBe('error');
    });
  });

  it('disables the Refit button while a fit is in flight', async () => {
    let resolveFit: ((v: unknown) => void) | undefined;
    mockCmd.mockImplementation(
      () => new Promise(resolve => {
        resolveFit = resolve;
      }),
    );
    render(
      <AdvisorPanel advisorSignals={[makeSignal()]} disagreement={null} />,
    );
    const button = screen.getByTestId('refit-button');
    fireEvent.click(button);
    await waitFor(() =>
      expect((button as HTMLButtonElement).disabled).toBe(true),
    );
    // Clean up the pending promise so the test doesn't leak.
    resolveFit?.({ total_candidates: 0, curves_produced: 0, fits: [] });
  });
});
