// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { isAbstentionSynthesis } from './briefing-synthesis-helpers';
import { adaptSnapshotItems } from './snapshot-adapter';
import { AttentionCards } from './AttentionCards';
import { IntelligenceFeed } from './IntelligenceFeed';
import type { SourceRelevance } from '../../types';
import type { InstantBriefingSnapshot } from '../../store/types';

interface InstantSnapshotPanelProps {
  snapshot: InstantBriefingSnapshot;
}

// The cold-boot panel is read-only — nothing here mutates state. A shared no-op
// keeps the interactive zone components' prop contracts satisfied without
// wiring Save/Dismiss/click handlers that would act on historical ids.
const noop = (_item: SourceRelevance) => { /* read-only cold-boot render */ };
const noopVoid = () => { /* read-only cold-boot render */ };

/**
 * Sovereign Cold Boot — instant first paint of yesterday's briefing while
 * fresh intelligence loads in the background.
 *
 * Renders through the SAME three-zone components as the live briefing
 * (`AttentionCards` + `IntelligenceFeed`), in read-only mode. This is the whole
 * point: the cold-boot paint and the live paint share one presentation, so the
 * hand-off is content sharpening in place — not a jarring swap from an old
 * prose-and-list layout to the redesigned card view. There is exactly one
 * briefing surface; the two render paths cannot visually drift.
 */
export const InstantSnapshotPanel = memo(function InstantSnapshotPanel({
  snapshot,
}: InstantSnapshotPanelProps) {
  const { t } = useTranslation();

  // A cached *abstention* ("Low signal — no noteworthy intelligence overnight")
  // carries nothing worth pre-painting. Echoing yesterday's verdict dead-center
  // while a fresh analysis is already in flight reads as a definitive "nothing's
  // happening" when the truth is "today's scan hasn't landed yet" — and asserts a
  // verdict the system hasn't re-derived (violates Accurate-first). So when the
  // snapshot is an abstention we render an honest *working* state instead; the real
  // low-signal verdict, if today warrants it, surfaces once live analysis completes.
  const isAbstention = isAbstentionSynthesis(snapshot.synthesis);

  const { signalItems, topItems, all } = useMemo(
    () => adaptSnapshotItems(snapshot.items),
    [snapshot.items],
  );
  const signalIds = useMemo(
    () => new Set(signalItems.map(s => s.id)),
    [signalItems],
  );

  if (isAbstention) {
    return (
      <section aria-label={t('briefing.dailyOverview')} className="bg-bg-primary rounded-lg">
        <div className="py-10 flex items-center justify-center gap-2.5">
          <span className="inline-block w-1.5 h-1.5 rounded-full bg-[#D4AF37] animate-pulse" />
          <p className="text-xs text-text-muted italic">
            {t('briefing.coldBootScanning', "Reading today's sources for new intelligence…")}
          </p>
        </div>
      </section>
    );
  }

  const itemCount = snapshot.totalRelevant || all.length;

  return (
    <section aria-label={t('briefing.dailyOverview')} className="bg-bg-primary rounded-lg space-y-5">
      {/* Zone 1 — cached pulse. Mirrors the live PulseSummary layout (dot +
          one-line summary + timestamp) but with a pulsing gold dot marking the
          "refreshing in background" state, so the shape matches the live view
          and the only change on hand-off is the sentence sharpening. */}
      <div className="relative px-5 py-4">
        <div className="flex items-center gap-3">
          <span
            className="w-2 h-2 rounded-full flex-shrink-0 bg-[#D4AF37] animate-pulse"
            title={t('briefing.refreshingInBackground', 'Refreshing intelligence in background')}
          />
          <div className="flex-1">
            <p className="text-sm leading-relaxed text-text-secondary">
              {t('briefing.cachedPulse', '{{count}} items from your last brief — refreshing intelligence…', { count: itemCount })}
            </p>
          </div>
          <div className="flex-shrink-0 ms-auto text-[10px] text-text-muted">
            {snapshot.generatedAtDisplay}
          </div>
        </div>
      </div>

      {/* Zone 2 — attention cards (read-only) */}
      <AttentionCards
        signalItems={signalItems}
        topItems={topItems}
        feedbackGiven={{}}
        onSave={noop}
        onDismiss={noop}
        onRecordClick={noop}
        readOnly
      />

      {/* Zone 3 — the feed (read-only) */}
      <IntelligenceFeed
        results={all}
        feedbackGiven={{}}
        signalIds={signalIds}
        onSave={noop}
        onDismiss={noop}
        onRecordClick={noop}
        onViewAll={noopVoid}
        readOnly
      />
    </section>
  );
});
