// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { AiAssessmentStatus } from './AiAssessmentStatus';

// Interpolation-aware t(): "key|model|age" so the test can see what was said.
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { model?: string; age?: string }) =>
      opts?.model !== undefined ? `${key}|${opts.model}|${opts.age ?? ''}` : key,
    i18n: { language: 'en', changeLanguage: vi.fn() },
  }),
}));

const HOUR = 3_600_000;

// Audit L4 (2026-10-12): Blind Spots showed a 3.5-day-old cached verdict as
// "Assessed by claude-sonnet-5" (no age, `stale` unsurfaced) beside
// "Assessment failed".
describe('AiAssessmentStatus', () => {
  it('a current verdict says who assessed it and how long ago', () => {
    render(<AiAssessmentStatus shown={{ model: 'claude-sonnet-5', assessedAt: Date.now() - 9 * HOUR, stale: false }} loading={false} error={null} />);
    const label = screen.getByTestId('blindspots-ai-assessed').textContent ?? '';
    expect(label.startsWith('blindspots.ai.assessedAgo|claude-sonnet-5|')).toBe(true);
    expect(label).toMatch(/9 hours ago/);
  });

  it('a stale persisted verdict is labelled earlier, with the reason', () => {
    render(<AiAssessmentStatus shown={{ model: 'claude-sonnet-5', assessedAt: Date.now() - 84 * HOUR, stale: true }} loading={false} error={null} />);
    const label = screen.getByTestId('blindspots-ai-assessed').textContent ?? '';
    expect(label.startsWith('blindspots.ai.earlierChanged|claude-sonnet-5|')).toBe(true);
    expect(label).toMatch(/4 days ago/);
  });

  it('a failed re-run never leaves the old verdict reading as current', () => {
    render(<AiAssessmentStatus shown={{ model: 'claude-sonnet-5', assessedAt: Date.now() - 84 * HOUR, stale: false }} loading={false} error="provider 500" />);
    expect(screen.getByText('blindspots.ai.failed')).toBeInTheDocument();
    const label = screen.getByTestId('blindspots-ai-assessed').textContent ?? '';
    expect(label.startsWith('blindspots.ai.earlier|')).toBe(true);
    expect(screen.queryByText(/assessedAgo/)).toBeNull();
  });

  it('accepts the bigint ts-rs types assessed_at as', () => {
    render(<AiAssessmentStatus shown={{ model: 'm', assessedAt: BigInt(Date.now() - 2 * HOUR), stale: false }} loading={false} error={null} />);
    expect(screen.getByTestId('blindspots-ai-assessed').textContent).toMatch(/2 hours ago/);
  });

  it('says nothing while an assessment is running', () => {
    const { container } = render(<AiAssessmentStatus shown={{ model: 'm', assessedAt: Date.now(), stale: true }} loading={true} error={null} />);
    expect(container.textContent).toBe('');
  });

  it('no model configured: the add-a-key hint, no failure', () => {
    render(<AiAssessmentStatus shown={null} loading={false} error="no_llm" />);
    expect(screen.getByText('blindspots.ai.noModel')).toBeInTheDocument();
    expect(screen.queryByText('blindspots.ai.failed')).toBeNull();
  });
});
