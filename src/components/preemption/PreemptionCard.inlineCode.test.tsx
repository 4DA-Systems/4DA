// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/react';
import { ItemCard, truncateAt } from './PreemptionCard';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));
vi.mock('../../lib/commands', () => ({
  cmd: vi.fn(() => Promise.resolve(null)),
}));
vi.mock('../../lib/trust-feedback', () => ({
  recordTrustEvent: vi.fn(),
}));

const makeItem = (explanation: string): EvidenceItem => ({
  id: 'plan-form-data',
  kind: 'alert',
  title: 'Upgrade form-data to 4.0.6',
  explanation,
  urgency: 'high',
  confidence: { value: 0.95, provenance: 'osv_verified', sample_size: null },
  reversibility: null,
  evidence: [],
  affected_deps: ['form-data'],
  affected_projects: [],
  suggested_actions: [],
  precedents: [],
  refutation_condition: null,
  lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false, other_build_target: false, upgrade_plan: true, no_coverage: false },
  created_at: BigInt(Date.now()),
  expires_at: null,
});

const surfacedRef = { current: new Set<string>() } as React.RefObject<Set<string>>;

function renderCard(explanation: string) {
  return render(<ItemCard item={makeItem(explanation)} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);
}

describe('ItemCard explanation inline code', () => {
  it('renders a backticked fix command as <code>, with no literal backticks', () => {
    const { container } = renderCard(
      'Fixable now via a direct dependency bump where it is declared: `npm install form-data@4.0.6 --save-exact` in the web project.',
    );
    const code = container.querySelector('p code');
    expect(code?.textContent).toBe('npm install form-data@4.0.6 --save-exact');
    expect(container.querySelector('p')?.textContent).not.toContain('`');
  });

  it('never injects HTML from the explanation', () => {
    const { container } = renderCard('Run `<img src=x onerror=alert(1)>` then <b>bold</b>');
    expect(container.querySelector('img')).toBeNull();
    expect(container.querySelector('p b')).toBeNull();
    expect(container.querySelector('p code')?.textContent).toBe('<img src=x onerror=alert(1)>');
    expect(container.querySelector('p')?.textContent).toContain('<b>bold</b>');
  });

  it('renders markdown links in card prose as plain text, not anchors', () => {
    const { container } = renderCard('See [the advisory](https://example.com/ghsa) for details.');
    expect(container.querySelector('p a')).toBeNull();
    expect(container.querySelector('p')?.textContent).toContain('the advisory');
  });

  it('keeps a code span intact when the collapsed explanation is truncated inside it', () => {
    const lead = 'x'.repeat(250);
    const { container } = renderCard(`${lead} \`npm install form-data@4.0.6 --save-exact\` in project`);
    const p = container.querySelector('p');
    expect(p?.querySelector('code')).not.toBeNull();
    expect(p?.textContent).not.toContain('`');
  });
});

describe('truncateAt', () => {
  it('closes a code span the cut lands inside', () => {
    // Cut at the last space inside the limit ("run `npm"), then the span closes.
    expect(truncateAt('run `npm install foo` now', 10)).toBe('run `npm`…');
  });

  it('leaves balanced text alone', () => {
    expect(truncateAt('run `npm` and more words here', 14)).toBe('run `npm` and…');
    expect(truncateAt('short', 10)).toBe('short');
  });
});
