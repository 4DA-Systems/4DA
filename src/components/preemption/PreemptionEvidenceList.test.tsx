// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';

import { EvidenceList } from './PreemptionEvidenceList';

const cite = (title: string, freshness_days: number) => ({
  source: 'osv-advisory', title, url: null, freshness_days, relevance_note: '',
});

// Fresh-profile audit E9: 2023 advisories were labelled "today" because the
// citation age was a 0.0 placeholder. An unknown age (negative) shows no
// label; a known one shows its age.
describe('EvidenceList citation age', () => {
  it('shows no age for an advisory whose publish date is unknown', () => {
    render(<EvidenceList evidence={[cite('GHSA-old', -1)]} hiddenExtra={0} />);
    expect(screen.getByText('GHSA-old')).toBeInTheDocument();
    expect(screen.queryByText('preemption.freshness.today')).toBeNull();
  });

  it('shows the age of a dated advisory', () => {
    render(<EvidenceList evidence={[cite('GHSA-dated', 900)]} hiddenExtra={0} />);
    expect(screen.getByText('preemption.freshness.monthsAgo')).toBeInTheDocument();
  });
});

// Advisory titles arrive from OSV in Markdown ("panic safety in `LruCache::pop()`");
// live 2026-11-07 the worklist showed the backticks raw. They render as code now,
// and the hover title keeps the original text.
describe('EvidenceList advisory titles', () => {
  it('renders inline code in a title as <code>, with no stray backticks', () => {
    const title = 'Potential use-after-free due to lack of panic safety in `LruCache::pop()`';
    const { container } = render(<EvidenceList evidence={[cite(title, 30)]} hiddenExtra={0} />);
    const code = container.querySelector('code');
    expect(code?.textContent).toBe('LruCache::pop()');
    expect(container.textContent).not.toContain('`');
    expect(container.querySelector('[title]')?.getAttribute('title')).toBe(title);
  });

  it('never turns title text into HTML', () => {
    const { container } = render(
      <EvidenceList evidence={[cite('<img src=x onerror=alert(1)> `x`', 30)]} hiddenExtra={0} />,
    );
    expect(container.querySelector('img')).toBeNull();
    expect(container.textContent).toContain('<img src=x onerror=alert(1)>');
  });
});
