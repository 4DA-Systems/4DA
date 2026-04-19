// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { StackHealthBar, type StackHealth, type TechHealthEntry } from './StackHealthBar';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (opts) {
        let result = key;
        for (const [k, v] of Object.entries(opts)) {
          result = result.replace(`{{${k}}}`, String(v));
        }
        return result;
      }
      return key;
    },
  }),
}));

function makeTech(overrides: Partial<TechHealthEntry> = {}): TechHealthEntry {
  return {
    name: 'Rust',
    category: 'language',
    status: 'healthy',
    signal_count_7d: 12,
    days_since_engagement: 1,
    has_knowledge_gap: false,
    ...overrides,
  };
}

function makeHealth(overrides: Partial<StackHealth> = {}): StackHealth {
  return {
    technologies: [makeTech()],
    stack_score: 82,
    signals_this_week: 25,
    suggested_queries: [],
    missed_signals: { total_count: 0, critical_count: 0, high_count: 0, example_titles: [] },
    ...overrides,
  };
}

describe('StackHealthBar', () => {
  const defaultOnQuery = vi.fn();

  it('returns null when health is null', () => {
    const { container } = render(
      <StackHealthBar health={null} onSuggestedQuery={defaultOnQuery} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('renders tech pills for each technology', () => {
    const health = makeHealth({
      technologies: [
        makeTech({ name: 'Rust', status: 'healthy' }),
        makeTech({ name: 'TypeScript', status: 'attention' }),
        makeTech({ name: 'Python', status: 'stale' }),
      ],
    });
    render(<StackHealthBar health={health} onSuggestedQuery={defaultOnQuery} />);
    expect(screen.getByText('Rust')).toBeInTheDocument();
    expect(screen.getByText('TypeScript')).toBeInTheDocument();
    expect(screen.getByText('Python')).toBeInTheDocument();
  });

  it('shows stack score', () => {
    const health = makeHealth({ stack_score: 75 });
    render(<StackHealthBar health={health} onSuggestedQuery={defaultOnQuery} />);
    expect(screen.getByText('75/100')).toBeInTheDocument();
  });

  it('shows missed signals banner when total_count > 0', () => {
    const health = makeHealth({
      missed_signals: { total_count: 5, critical_count: 2, high_count: 3, example_titles: ['CVE-2024'] },
    });
    render(<StackHealthBar health={health} onSuggestedQuery={defaultOnQuery} />);
    expect(screen.getByText('search.missedSignals')).toBeInTheDocument();
  });

  it('shows example titles in missed signals details', () => {
    const health = makeHealth({
      missed_signals: {
        total_count: 10,
        critical_count: 1,
        high_count: 3,
        example_titles: ['CVE-2024-1234 in OpenSSL', 'React 19 breaking changes', 'SQLite 3.45 release'],
      },
    });
    render(<StackHealthBar health={health} onSuggestedQuery={defaultOnQuery} />);
    expect(screen.getByText('CVE-2024-1234 in OpenSSL')).toBeInTheDocument();
    expect(screen.getByText('React 19 breaking changes')).toBeInTheDocument();
    expect(screen.getByText('SQLite 3.45 release')).toBeInTheDocument();
    // Shows "+7 more" for remaining signals
    expect(screen.getByText('search.missedMore')).toBeInTheDocument();
  });

  it('does not show missed signals banner when total_count is 0', () => {
    const health = makeHealth({
      missed_signals: { total_count: 0, critical_count: 0, high_count: 0, example_titles: [] },
    });
    render(<StackHealthBar health={health} onSuggestedQuery={defaultOnQuery} />);
    expect(screen.queryByText(/search\.missedSignals/)).not.toBeInTheDocument();
  });

  it('calls onSuggestedQuery when chip clicked', () => {
    const onQuery = vi.fn();
    const health = makeHealth({
      suggested_queries: ['Rust async patterns', 'Tauri 2.0 migration'],
    });
    render(<StackHealthBar health={health} onSuggestedQuery={onQuery} />);

    fireEvent.click(screen.getByText('Rust async patterns'));
    expect(onQuery).toHaveBeenCalledWith('Rust async patterns');

    fireEvent.click(screen.getByText('Tauri 2.0 migration'));
    expect(onQuery).toHaveBeenCalledWith('Tauri 2.0 migration');
    expect(onQuery).toHaveBeenCalledTimes(2);
  });

  it('color codes status correctly via inline style', () => {
    const health = makeHealth({
      technologies: [
        makeTech({ name: 'Healthy', status: 'healthy' }),
        makeTech({ name: 'Critical', status: 'critical' }),
      ],
    });
    const { container } = render(
      <StackHealthBar health={health} onSuggestedQuery={defaultOnQuery} />,
    );
    // Status icons get inline color styles
    const coloredSpans = container.querySelectorAll('span[style]');
    const colors = Array.from(coloredSpans).map((el) => (el as HTMLElement).style.color);
    expect(colors).toContain('rgb(34, 197, 94)'); // #22C55E -> healthy
    expect(colors).toContain('rgb(239, 68, 68)'); // #EF4444 -> critical
  });
});
