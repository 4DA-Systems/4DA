// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { SourceRelevance } from '../../types';
import { ScoreBreakdownRow } from './ScoreBreakdownRow';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: string | { defaultValue?: string; count?: number; total?: number }) => {
      if (typeof options === 'string') return options;
      if (options?.defaultValue) return options.defaultValue;
      return key;
    },
  }),
}));

vi.mock('../ConfidenceIndicator', () => ({
  ConfidenceIndicator: () => null,
}));

vi.mock('../../config/sources', () => ({
  getSourceLabel: (source: string) => source,
}));

function makeItem(overrides: Partial<SourceRelevance> = {}): SourceRelevance {
  return {
    id: 1,
    title: 'npm: react v19.2.5',
    url: 'https://example.com/react',
    top_score: 0.8,
    matches: [],
    relevant: true,
    source_type: 'npm',
    score_breakdown: {
      context_score: 0.66,
      interest_score: 0.07,
      ace_boost: 0.15,
      affinity_mult: 1.2,
      anti_penalty: 0.8,
      freshness_mult: 1.1,
      confidence_by_signal: {},
      signal_count: 3,
      confirmed_signals: ['context', 'interest', 'ace'],
    },
    ...overrides,
  };
}

describe('ScoreBreakdownRow', () => {
  it('renders readable fallback copy when score translation keys are missing', () => {
    render(
      <ScoreBreakdownRow
        item={makeItem()}
        isTopPick={true}
        isHighConfidence={false}
      />,
    );

    expect(screen.getByText('Strong match')).toBeInTheDocument();
    expect(screen.getByText('context 66%')).toBeInTheDocument();
    expect(screen.getByText('interest 7%')).toBeInTheDocument();
    expect(screen.getByText('recent work +15%')).toBeInTheDocument();
    expect(screen.getByText('affinity x1.2')).toBeInTheDocument();
    expect(screen.getByText('penalty x0.8')).toBeInTheDocument();
    expect(screen.getByText('fresh +10%')).toBeInTheDocument();
  });
});
