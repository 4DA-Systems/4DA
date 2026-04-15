/**
 * AdvisorPanel — Intelligence Mesh Phase 7b component test.
 *
 * Validates the receipts panel shows advisor identity, normalized scores,
 * reasoning, and kind-specific disagreement narrative. The
 * "uncalibrated tag" case is load-bearing: until Phase 5 lands real
 * calibration curves, every advisor is marked pre-mesh so users know
 * the number isn't anchored yet.
 */
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { AdvisorPanel } from './AdvisorPanel';
import type { AdvisorSignal } from '../../types/analysis';

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

  it('hides the uncalibrated tag when a real calibration_id is present', () => {
    render(
      <AdvisorPanel
        advisorSignals={[
          makeSignal({ calibration_id: 'judge-llama3.2-cal-v1-2026-05-01' }),
        ]}
        disagreement={null}
      />,
    );
    expect(screen.queryByText('uncalibrated')).toBeNull();
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
});
