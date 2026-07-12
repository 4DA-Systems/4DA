// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { InstantSnapshotPanel } from './InstantSnapshotPanel';
import type { InstantBriefingSnapshot } from '../../store/types';

// react-i18next is mocked globally in src/test/setup.ts so that `t(key, default)`
// returns the KEY. That lets these tests assert on stable i18n keys rather than
// on copy that the creative director may reword.

function makeSnapshot(overrides: Partial<InstantBriefingSnapshot> = {}): InstantBriefingSnapshot {
  return {
    version: 1,
    generatedAtUnix: 1781791463,
    generatedAtDisplay: 'Fri Jun 19, 12:04 AM',
    title: '4DA Intelligence Briefing',
    items: [
      { title: 'axios@1.12.2: 24 known vulnerabilities', sourceType: 'osv', score: 0.91 },
      { title: 'I Learned React. Then I Realized…', sourceType: 'devto', score: 0.89 },
    ],
    totalRelevant: 2,
    synthesis: null,
    wisdomSynthesis: null,
    ...overrides,
  };
}

describe('InstantSnapshotPanel', () => {
  // The cold-boot panel pre-paints yesterday's briefing while a fresh analysis
  // runs. When yesterday's snapshot was an ABSTENTION ("Low signal — no
  // noteworthy intelligence overnight"), echoing that verdict dead-center reads
  // as a definitive "nothing is happening" when the truth is "today's scan
  // hasn't landed yet". The panel must show an honest working state instead.
  describe('cached abstention while freshening', () => {
    const abstention = makeSnapshot({
      synthesis: 'Low signal -- no noteworthy intelligence overnight.',
    });

    it('shows the scanning state, not the stale abstention verdict', () => {
      render(<InstantSnapshotPanel snapshot={abstention} />);
      expect(screen.getByText('briefing.coldBootScanning')).toBeInTheDocument();
      expect(screen.queryByText(/low signal/i)).not.toBeInTheDocument();
      expect(screen.queryByText(/no noteworthy intelligence/i)).not.toBeInTheDocument();
    });

    it('suppresses the contradictory "cached briefing" footer', () => {
      // Nothing cached is on screen in the abstention case, so the
      // "Cached briefing — fresh intelligence loading…" footer would lie.
      render(<InstantSnapshotPanel snapshot={abstention} />);
      expect(screen.queryByText('briefing.cachedFreshening')).not.toBeInTheDocument();
    });

    it('does not render a source-items list for an abstention', () => {
      // An abstention carries items in its payload, but the brief is saying
      // "nothing worth surfacing" — listing them would undermine that.
      const withItems = makeSnapshot({
        synthesis: 'Low signal -- no noteworthy intelligence overnight.',
      });
      render(<InstantSnapshotPanel snapshot={withItems} />);
      expect(screen.queryByText(/axios@1.12.2/)).not.toBeInTheDocument();
    });

    it('tolerates dash and "no new" variants of the abstention marker', () => {
      render(
        <InstantSnapshotPanel
          snapshot={makeSnapshot({ synthesis: 'Low signal — no new intelligence overnight.' })}
        />,
      );
      expect(screen.getByText('briefing.coldBootScanning')).toBeInTheDocument();
    });
  });

  describe('cached real briefing while freshening', () => {
    const real = makeSnapshot({
      synthesis: 'SITUATION\nA real briefing with substance.\n\nPRIORITY\nPatch axios.',
    });

    it('renders the item titles through the live three-zone components', () => {
      render(<InstantSnapshotPanel snapshot={real} />);
      // Items surface through the shared AttentionCards / IntelligenceFeed
      // components (a featured item appears as a card AND a feed row), so there
      // may be more than one match — assert at least one.
      expect(screen.getAllByText('axios@1.12.2: 24 known vulnerabilities').length).toBeGreaterThan(0);
      expect(screen.getAllByText('I Learned React. Then I Realized…').length).toBeGreaterThan(0);
    });

    it('shows the cached "refreshing" pulse with the snapshot timestamp', () => {
      render(<InstantSnapshotPanel snapshot={real} />);
      expect(screen.getByText('briefing.cachedPulse')).toBeInTheDocument();
      expect(screen.getByText('Fri Jun 19, 12:04 AM')).toBeInTheDocument();
    });

    it('is read-only — no Save/Dismiss mutations on historical items', () => {
      // The cold-boot snapshot is yesterday's data; acting on it would mutate
      // state for stale ids. Read-only mode suppresses those affordances.
      render(<InstantSnapshotPanel snapshot={real} />);
      expect(screen.queryByText('action.save')).not.toBeInTheDocument();
      expect(screen.queryByText('action.dismiss')).not.toBeInTheDocument();
    });

    it('does not show the scanning state when real content exists', () => {
      render(<InstantSnapshotPanel snapshot={real} />);
      expect(screen.queryByText('briefing.coldBootScanning')).not.toBeInTheDocument();
    });

    it('keeps a Read affordance for items that carry a url', () => {
      const withUrl = makeSnapshot({
        items: [{ title: 'Patchable CVE', sourceType: 'osv', score: 0.9, url: 'https://example.com/adv' }],
      });
      render(<InstantSnapshotPanel snapshot={withUrl} />);
      expect(screen.getAllByText('briefing.read').length).toBeGreaterThan(0);
    });
  });

  // DRIFT GUARD — the whole point of the cold-boot rewrite is that it composes
  // through the SAME zone components as the live briefing (AttentionCards +
  // IntelligenceFeed), so the two render paths cannot visually diverge again.
  // `feed.title` is IntelligenceFeed's review-queue heading: if a future change
  // rips the shared feed out of the cold-boot panel (re-opening the drift this
  // fix closed), this test fails. Keep it in lock-step with the live view's use
  // of the same components in BriefingContentPanel.
  describe('shares the live three-zone shell (drift guard)', () => {
    it('renders through IntelligenceFeed — the review-queue heading is present', () => {
      const real = makeSnapshot({
        items: [
          { title: 'axios@1.12.2: 24 known vulnerabilities', sourceType: 'osv', score: 0.91 },
          { title: 'Feed-only item', sourceType: 'hackernews', score: 0.72 },
          { title: 'Another feed item', sourceType: 'devto', score: 0.66 },
        ],
        totalRelevant: 3,
      });
      render(<InstantSnapshotPanel snapshot={real} />);
      expect(screen.getByText('feed.title')).toBeInTheDocument();
    });
  });
});
