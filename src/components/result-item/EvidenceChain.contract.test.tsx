// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Rendering contract for the explanation evidence chain (Wave 8):
//   - subtitle = top factor display
//   - chips = factors 2..4 (named), "+N more" only as a suffix to named chips
//   - collapsed and expanded views read the SAME chain
//   - bare count strings ("N signals confirmed", "N reasons") never render
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { ExplanationFactor, SourceRelevance } from '../../types';
import { EvidenceChain } from './EvidenceChain';
import { ResultItemCollapsed } from './ResultItemCollapsed';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: string | { defaultValue?: string; count?: number }) => {
      if (typeof options === 'string') return options;
      if (key === 'result.moreFactors' && options && typeof options === 'object') {
        return `+${options.count} more`;
      }
      if (options && typeof options === 'object' && options.defaultValue) return options.defaultValue;
      return key;
    },
  }),
}));

vi.mock('../ContentTranslationProvider', () => ({
  useTranslatedContent: () => ({ getTranslated: (_id: string, title: string) => title }),
}));

vi.mock('../../config/sources', () => ({
  getSourceLabel: (source: string) => source,
  getSourceColorClass: () => '',
}));

vi.mock('../../lib/commands', () => ({
  cmd: vi.fn().mockResolvedValue(undefined),
}));

function factor(overrides: Partial<ExplanationFactor> = {}): ExplanationFactor {
  return {
    kind: 'DependencyMatch',
    display: 'Names your dependency axios',
    evidence: 'axios (direct, installed v1.6.2) — named in the item text',
    weight_share: 0.5,
    ...overrides,
  };
}

const CHAIN: ExplanationFactor[] = [
  factor(),
  factor({ kind: 'ContextMatch', display: 'Uses react (your stack)', evidence: 'your declared stack: react', weight_share: 0.25 }),
  factor({ kind: 'InterestMatch', display: 'Matches interest: webassembly', evidence: "'webassembly' in the title", weight_share: 0.12 }),
  factor({ kind: 'TopicMatch', display: 'Overlaps your recent work: sqlite', evidence: 'active topics from your commits: sqlite', weight_share: 0.08 }),
  factor({ kind: 'SkillGap', display: 'Closes skill gap: kubernetes', evidence: 'you use kubernetes but have not engaged with recent updates', weight_share: 0.05 }),
];

function makeItem(overrides: Partial<SourceRelevance> = {}): SourceRelevance {
  return {
    id: 1,
    title: 'axios 1.7 fixes SSRF',
    url: null,
    top_score: 0.8,
    matches: [],
    relevant: true,
    source_type: 'hackernews',
    explanation: CHAIN[0]?.display,
    score_breakdown: {
      context_score: 0.4,
      interest_score: 0.2,
      ace_boost: 0.1,
      affinity_mult: 1.0,
      anti_penalty: 0,
      confidence_by_signal: {},
      signal_count: 3,
      explanation_factors: CHAIN,
    },
    ...overrides,
  };
}

function renderCollapsed(item: SourceRelevance, isExpanded = false) {
  return render(
    <ResultItemCollapsed
      item={item}
      isExpanded={isExpanded}
      onToggleExpand={() => {}}
      feedback={undefined}
      fallbackReason=""
    />,
  );
}

describe('collapsed rendering contract', () => {
  it('renders the top 3 factors as named chips (strongest evidence leads)', () => {
    renderCollapsed(makeItem());
    expect(screen.getByText('Names your dependency axios')).toBeInTheDocument();
    expect(screen.getByText('Uses react (your stack)')).toBeInTheDocument();
    expect(screen.getByText('Matches interest: webassembly')).toBeInTheDocument();
  });

  it('renders the top factor as the FIRST chip (not hidden until expand)', () => {
    const { container } = renderCollapsed(makeItem());
    const text = container.textContent ?? '';
    // factor[0] must appear, and lead the other chips.
    expect(screen.getByText('Names your dependency axios')).toBeInTheDocument();
    expect(text.indexOf('Names your dependency axios')).toBeLessThan(
      text.indexOf('Uses react (your stack)'),
    );
  });

  it('overflows factors beyond the top 3 into the "+N more" suffix', () => {
    renderCollapsed(makeItem());
    // CHAIN has 5 factors; 3 lead as chips, 2 overflow.
    expect(screen.queryByText('Overlaps your recent work: sqlite')).not.toBeInTheDocument();
    const more = screen.getByText('+2 more');
    expect(more).toBeInTheDocument();
  });

  it('renders "+N more" only as a suffix after named chips', () => {
    const { container } = renderCollapsed(makeItem());
    const text = container.textContent ?? '';
    // The named chips must precede the +N suffix in the DOM.
    expect(text.indexOf('Names your dependency axios')).toBeLessThan(text.indexOf('+2 more'));
  });

  it('never renders bare count strings', () => {
    const { container } = renderCollapsed(makeItem());
    expect(container.textContent).not.toContain('signals confirmed');
    expect(container.textContent).not.toContain('signalsConfirmed');
  });

  it('omits "+N more" when the chain has 3 or fewer factors', () => {
    const item = makeItem();
    item.score_breakdown!.explanation_factors = CHAIN.slice(0, 3);
    const { container } = renderCollapsed(item);
    expect(container.textContent).not.toContain('more');
  });

  it('falls back to legacy generic chips only when no chain exists', () => {
    const item = makeItem();
    item.score_breakdown!.explanation_factors = [];
    renderCollapsed(item);
    // Legacy chip keys (i18n keys echoed by the mock) may appear; named
    // factor chips must not.
    expect(screen.queryByText('Uses react (your stack)')).not.toBeInTheDocument();
  });
});

describe('expanded chain (EvidenceChain)', () => {
  it('renders every factor with display and evidence — same chain as collapsed', () => {
    render(<EvidenceChain factors={CHAIN} />);
    for (const f of CHAIN) {
      expect(screen.getByText(f.display)).toBeInTheDocument();
      expect(screen.getByText(f.evidence)).toBeInTheDocument();
    }
  });

  it('renders nothing for an empty chain', () => {
    const { container } = render(<EvidenceChain factors={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it('sizes weight bars by weight_share', () => {
    const { container } = render(
      <EvidenceChain factors={[factor({ weight_share: 0.5 })]} />,
    );
    const bar = container.querySelector('li span span') as HTMLElement;
    expect(bar?.style.width).toBe('50%');
  });
});
