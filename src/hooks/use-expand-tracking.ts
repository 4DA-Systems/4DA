import { useRef, useEffect } from 'react';
import { cmd } from '../lib/commands';

/**
 * Classifies a click interaction into an InteractionPattern from available
 * behavioral data.
 *
 * Intelligence Mesh Phase 6a — honest implicit feedback. A naive scorer
 * treats every long dwell as strong engagement, which is the single biggest
 * failure mode of passive-feedback systems: it cannot tell a user reading
 * confused from a user reading engaged. This classifier distinguishes the
 * two (as best we can from current data) so the Learned axis doesn't
 * compound confusion into fake affinity.
 *
 * With only dwell data today, the bands below are deliberately conservative.
 * When Phase 6b adds scroll-to-bottom detection and direction-change
 * counting, we'll be able to identify Reread and Completed precisely.
 * Until then: Bounced / Scanned / Engaged / Abandoned from dwell alone.
 *
 * Backend maps these snake_case strings to `InteractionPattern` variants
 * in src-tauri/src/ace/behavior.rs. Any rename there must update this file.
 */
export type InteractionPattern =
  | 'bounced'
  | 'scanned'
  | 'engaged'
  | 'completed'
  | 'reread'
  | 'abandoned';

export function classifyInteractionPattern(
  dwellSeconds: number,
): InteractionPattern {
  // < 4s: too short to have read anything. Likely opened, looked, closed.
  if (dwellSeconds < 4) return 'bounced';
  // 4-20s: skimmed, possibly triaged without reading in depth.
  if (dwellSeconds < 20) return 'scanned';
  // 20-120s: range where a focused developer actually reads a feed item.
  if (dwellSeconds <= 120) return 'engaged';
  // > 120s with no other engagement signal: item expanded, window lost
  // focus, tab left open. Not real engagement — don't reward it.
  return 'abandoned';
}

/**
 * Emit both click (dwell) and engagement_complete signals for an expand session.
 * Called on collapse and unmount to capture the full engagement depth.
 */
function emitExpandSignals(
  itemId: number,
  sourceType: string,
  itemTopics: string[],
  dwellSeconds: number,
  label: string,
) {
  const pattern = classifyInteractionPattern(dwellSeconds);

  // Click + dwell + pattern. Pattern is the load-bearing field — a bounced
  // click with 20s dwell (user stared confused) must NOT score as positive.
  cmd('ace_record_interaction', {
    item_id: itemId,
    action_type: 'click',
    action_data: JSON.stringify({
      type: 'click',
      dwell_time_seconds: dwellSeconds,
      pattern,
    }),
    item_topics: itemTopics,
    item_source: sourceType,
  }).catch((e) => console.debug(`[expand-tracking] ${label} click:`, e));

  // Layer 2: engagement_complete — deep engagement depth signal
  // Expanded items show full content, so scroll_depth_pct = 1.0
  cmd('ace_record_interaction', {
    item_id: itemId,
    action_type: 'engagement_complete',
    action_data: JSON.stringify({
      total_seconds: dwellSeconds,
      scroll_depth_pct: 1.0,
    }),
    item_topics: itemTopics,
    item_source: sourceType,
  }).catch((e) => console.debug(`[expand-tracking] ${label} engagement:`, e));
}

/**
 * Track dwell time when a content item is expanded.
 * On collapse or unmount, emits a 'click' interaction with dwell_time_seconds
 * AND an 'engagement_complete' signal for Layer 2 behavioral microlearning.
 * This captures the strongest implicit signal: the user chose to read deeper.
 */
export function useExpandTracking(
  itemId: number,
  sourceType: string,
  isExpanded: boolean,
  itemTopics: string[],
) {
  const expandedAt = useRef<number | null>(null);
  const emitted = useRef(false);

  useEffect(() => {
    if (isExpanded) {
      expandedAt.current = Date.now();
      emitted.current = false;
    } else if (expandedAt.current && !emitted.current) {
      // Collapsed — emit dwell + engagement signals
      const dwellSeconds = Math.round((Date.now() - expandedAt.current) / 1000);
      if (dwellSeconds >= 1) {
        emitted.current = true;
        emitExpandSignals(itemId, sourceType, itemTopics, dwellSeconds, 'collapse');
      }
      expandedAt.current = null;
    }
  }, [isExpanded, itemId, sourceType, itemTopics]);

  // Also emit on unmount if still expanded
  useEffect(() => {
    return () => {
      if (expandedAt.current && !emitted.current) {
        const dwellSeconds = Math.round((Date.now() - expandedAt.current) / 1000);
        if (dwellSeconds >= 1) {
          emitted.current = true;
          emitExpandSignals(itemId, sourceType, itemTopics, dwellSeconds, 'unmount');
        }
      }
    };
  }, [itemId, sourceType, itemTopics]);
}
