/**
 * JudgesSplitBadge — Intelligence Mesh Phase 7 component test.
 *
 * Validates the three disagreement variants render with distinct
 * tooltips and the badge is suppressed when disagreement is null/undefined.
 * The per-kind tooltip test is the load-bearing one: it proves the UI
 * says different things for AdvisorSkeptical vs. AdvisorEnthusiastic vs.
 * AdvisorsInternal, which is what the "judges split" promise means.
 */
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { JudgesSplitBadge } from './JudgesSplitBadge';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const table: Record<string, string> = {
        'results.judgesSplit': 'Split',
        'results.judgesSplitSkeptical':
          'Pipeline rated this relevant; the LLM judge was skeptical.',
        'results.judgesSplitEnthusiastic':
          'Pipeline rated this lower; the LLM judge was enthusiastic.',
        'results.judgesSplitInternal':
          'Multiple advisors disagreed with each other.',
      };
      return table[key] ?? key;
    },
  }),
}));

describe('JudgesSplitBadge', () => {
  it('renders nothing when disagreement is null', () => {
    const { container } = render(<JudgesSplitBadge disagreement={null} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders nothing when disagreement is undefined', () => {
    const { container } = render(<JudgesSplitBadge disagreement={undefined} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders the Split label when disagreement is set', () => {
    render(<JudgesSplitBadge disagreement="AdvisorSkeptical" />);
    expect(screen.getByTestId('judges-split-badge')).toBeTruthy();
    expect(screen.getByText('Split')).toBeTruthy();
  });

  it('surfaces the skeptical variant tooltip + data attribute', () => {
    render(<JudgesSplitBadge disagreement="AdvisorSkeptical" />);
    const badge = screen.getByTestId('judges-split-badge');
    expect(badge.getAttribute('title')).toMatch(/skeptical/i);
    expect(badge.getAttribute('data-disagreement-kind')).toBe('AdvisorSkeptical');
  });

  it('surfaces the enthusiastic variant tooltip + data attribute', () => {
    render(<JudgesSplitBadge disagreement="AdvisorEnthusiastic" />);
    const badge = screen.getByTestId('judges-split-badge');
    expect(badge.getAttribute('title')).toMatch(/enthusiastic/i);
    expect(badge.getAttribute('data-disagreement-kind')).toBe('AdvisorEnthusiastic');
  });

  it('surfaces the internal-disagreement variant tooltip + data attribute', () => {
    render(<JudgesSplitBadge disagreement="AdvisorsInternal" />);
    const badge = screen.getByTestId('judges-split-badge');
    expect(badge.getAttribute('title')).toMatch(/disagreed with each other/i);
    expect(badge.getAttribute('data-disagreement-kind')).toBe('AdvisorsInternal');
  });

  it('exposes tooltip via aria-label for screen readers', () => {
    // Load-bearing: sighted users see the pill + tooltip, screen reader users
    // get the full explanation. Badge must not be invisible to assistive tech.
    render(<JudgesSplitBadge disagreement="AdvisorSkeptical" />);
    const badge = screen.getByTestId('judges-split-badge');
    expect(badge.getAttribute('aria-label')).toMatch(/skeptical/i);
  });
});
