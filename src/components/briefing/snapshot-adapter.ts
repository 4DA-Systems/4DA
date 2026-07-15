// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { InstantBriefingSnapshot } from '../../store/types';
import type { SourceRelevance } from '../../types';

/**
 * Adapt a cold-boot snapshot item into the `SourceRelevance` shape the live
 * briefing zones (`AttentionCards`, `IntelligenceFeed`) render.
 *
 * This is deliberately LOSSY and HONEST: a snapshot only carries what was
 * persisted (title, source, score, signal hints, url) — it has no
 * `score_breakdown`, `confidence`, `explanation`, or `matches`. We do NOT
 * fabricate those. The cards degrade gracefully on their absence (no necessity
 * highlight, relevance derived from `top_score`), which is exactly right: a
 * cached item should look like a real item minus the live-only affordances,
 * not like a real item with invented evidence.
 *
 * Ids are synthetic and NEGATIVE (`-(index + 1)`) so they can never collide
 * with a real live result id (always positive) — the cached panel is read-only,
 * so these ids are used only as React keys, never for feedback/interaction.
 */
function adaptItem(
  item: InstantBriefingSnapshot['items'][number],
  index: number,
): SourceRelevance {
  return {
    id: -(index + 1),
    title: item.title,
    url: item.url ?? null,
    top_score: item.score,
    matches: [],
    // Every item that made it into the brief was, by definition, relevant.
    relevant: true,
    source_type: item.sourceType,
    ...(item.signalType ? { signal_type: item.signalType } : {}),
    ...(item.signalPriority ? { signal_priority: item.signalPriority } : {}),
  };
}

export interface AdaptedSnapshot {
  /** Attention-worthy items (Zone 2 cards): carry a signal hint. */
  signalItems: SourceRelevance[];
  /** Remaining items, highest score first (fill Zone 2, then Zone 3 feed). */
  topItems: SourceRelevance[];
  /** All adapted items — the feed filters/splices from this. */
  all: SourceRelevance[];
}

/**
 * Split adapted snapshot items into the (signalItems, topItems, all) triple the
 * live zones consume, mirroring `use-briefing-derived.ts` EXACTLY so the
 * cold-boot paint and the live paint compose through identical component logic
 * (same featured-cards-plus-full-feed behaviour, no drift):
 *   - signalItems: critical/alert priority, capped at 3 (Zone 2 leads)
 *   - topItems: relevant, score >= 0.5, non-signal, score-sorted, capped at 8
 *   - all: everything (Zone 3 feed excludes only the signals, so featured top
 *     items intentionally also appear in the feed — matching the live view)
 */
export function adaptSnapshotItems(
  items: InstantBriefingSnapshot['items'],
): AdaptedSnapshot {
  const all = items.map(adaptItem);
  const signalItems = all
    .filter(r => r.signal_priority === 'critical' || r.signal_priority === 'alert')
    .slice(0, 3);
  const signalIds = new Set(signalItems.map(s => s.id));
  const topItems = all
    .filter(r => r.relevant && r.top_score >= 0.5 && !signalIds.has(r.id))
    .sort((a, b) => b.top_score - a.top_score)
    .slice(0, 8);
  return { signalItems, topItems, all };
}
