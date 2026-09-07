// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// The Brief's review queue reads the ONE surfaced-signal predicate
// (`isSurfacedSignal`: relevant AND not excluded), the same one the header
// chip reads. Until 2026-09-07 it read `relevant` alone, so the Brief's own
// rejections (`excluded_by = "brief:…"`, which keep `relevant = true` as an
// ordering verdict) and rows the durable verdict rejected were listed here —
// at the top, by rank — and counted in "view all" while the header did not
// count them.
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { IntelligenceFeed } from './IntelligenceFeed';
import { makeItem } from '../../test/factories';

vi.mock('../ContentTranslationProvider', () => ({
  useTranslatedContent: () => ({
    getTranslated: (_id: string, fallback: string) => fallback,
    requestTranslation: vi.fn(),
  }),
}));

const noop = () => {};

function renderFeed(results: ReturnType<typeof makeItem>[]) {
  return render(
    <IntelligenceFeed
      results={results}
      feedbackGiven={{}}
      signalIds={new Set()}
      onSave={noop}
      onDismiss={noop}
      onRecordClick={noop}
      onViewAll={noop}
    />,
  );
}

describe('IntelligenceFeed — one surfaced-signal predicate', () => {
  it('lists only surfaced signals: an excluded row is never shown, whatever its relevant flag', () => {
    renderFeed([
      makeItem({ id: 1, title: 'Announcing Rust 1.98', relevant: true, top_score: 0.9 }),
      // A Brief demotion (`excluded_by = "brief:…"` on the backend) keeps
      // `relevant = true`; the frontend type carries only the `excluded` flag.
      makeItem({
        id: 2,
        title: '2D Game Development: From Zero To Hero - Rust edition',
        relevant: true,
        excluded: true,
        top_score: 0.93,
      }),
      // A durable-verdict demotion (`verdict:llm_reject`) clears `relevant`.
      makeItem({
        id: 3,
        title: 'Rust has become a spiritual experience',
        relevant: false,
        excluded: true,
        top_score: 0.9,
      }),
      makeItem({ id: 4, title: 'A noise item', relevant: false, top_score: 0.2 }),
    ]);

    expect(screen.getByText('Announcing Rust 1.98')).toBeInTheDocument();
    expect(screen.queryByText(/2D Game Development/)).not.toBeInTheDocument();
    expect(screen.queryByText(/spiritual experience/)).not.toBeInTheDocument();
    expect(screen.queryByText('A noise item')).not.toBeInTheDocument();
  });

  it('counts "view all" over surfaced signals only, so a brief-demoted row cannot tip it', () => {
    const fifteen = Array.from({ length: 15 }, (_, i) =>
      makeItem({ id: 100 + i, title: `Surfaced item ${i}`, relevant: true, top_score: 0.8 }),
    );
    const briefDemoted = makeItem({
      id: 200,
      title: 'Brief-demoted row',
      relevant: true,
      excluded: true,
      top_score: 0.95,
    });

    // 15 surfaced + 1 brief-demoted: the old `relevant` count said 16 and
    // offered "view all"; the surfaced count says 15 and does not.
    const { unmount } = renderFeed([...fifteen, briefDemoted]);
    expect(screen.queryByText('feed.viewAll')).not.toBeInTheDocument();
    unmount();

    // A sixteenth SURFACED row does offer it.
    renderFeed([
      ...fifteen,
      makeItem({ id: 116, title: 'Surfaced item 16', relevant: true, top_score: 0.7 }),
    ]);
    expect(screen.getByText('feed.viewAll')).toBeInTheDocument();
  });

  it('renders nothing when no surfaced signal remains', () => {
    const { container } = renderFeed([
      makeItem({
        id: 1,
        title: 'Only a brief-demoted row',
        relevant: true,
        excluded: true,
        top_score: 0.9,
      }),
    ]);
    expect(container).toBeEmptyDOMElement();
  });
});
