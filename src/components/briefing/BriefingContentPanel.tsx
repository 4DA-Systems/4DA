// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useEffect, useMemo } from 'react';
import { PulseSummary } from './PulseSummary';
import { AttentionCards } from './AttentionCards';
import { IntelligenceFeed } from './IntelligenceFeed';
import { useTranslatedContent } from '../ContentTranslationProvider';
import type { SourceRelevance, SourceHealthStatus, FeedbackAction } from '../../types';
import type { BriefingState } from '../../store/types';

import type { ActiveView } from '../../store/types';

interface BriefingContentPanelProps {
  briefing: BriefingState;
  results: SourceRelevance[];
  feedbackGiven: Record<number, FeedbackAction>;
  sourceHealth: SourceHealthStatus[];
  signalItems: SourceRelevance[];
  topItems: SourceRelevance[];
  onSave: (item: SourceRelevance) => void;
  onDismiss: (item: SourceRelevance) => void;
  onRecordClick: (item: SourceRelevance) => void;
  setActiveView: (view: ActiveView) => void;
}

/**
 * Intelligence Hierarchy — 3 zones, no noise.
 *
 * Zone 1 (Pulse): One-sentence summary answering "What happened?"
 * Zone 2 (Attention): 3-5 action cards answering "What needs me?"
 * Zone 3 (Feed): Compact content list answering "What's interesting?"
 */
export const BriefingContentPanel = memo(function BriefingContentPanel({
  briefing,
  results,
  feedbackGiven,
  sourceHealth,
  signalItems,
  topItems,
  onSave,
  onDismiss,
  onRecordClick,
  setActiveView,
}: BriefingContentPanelProps) {
  const signalIds = useMemo(
    () => new Set(signalItems.map(s => s.id)),
    [signalItems],
  );

  // Request content translation for all visible items
  const { requestTranslation } = useTranslatedContent();
  useEffect(() => {
    const allItems = [...signalItems, ...topItems, ...results];
    if (allItems.length === 0) return;

    // Deduplicate by ID and request translation
    const seen = new Set<number>();
    const items = allItems
      .filter((item) => {
        if (seen.has(item.id)) return false;
        seen.add(item.id);
        return true;
      })
      .map((item) => ({ id: String(item.id), text: item.title }));

    requestTranslation(items);
  }, [results, signalItems, topItems, requestTranslation]);

  return (
    <>
      {/* Zone 1: The Pulse — one-sentence summary */}
      <PulseSummary
        results={results}
        sourceHealth={sourceHealth}
        briefing={briefing}
        signalCount={signalItems.length}
        topCount={topItems.length}
      />

      {/* Zone 2: Attention — action cards */}
      <AttentionCards
        signalItems={signalItems}
        topItems={topItems}
        feedbackGiven={feedbackGiven}
        onSave={onSave}
        onDismiss={onDismiss}
        onRecordClick={onRecordClick}
      />

      {/* Zone 3: The Feed — compact content items */}
      <IntelligenceFeed
        results={results}
        feedbackGiven={feedbackGiven}
        signalIds={signalIds}
        onSave={onSave}
        onDismiss={onDismiss}
        onRecordClick={onRecordClick}
        onViewAll={() => setActiveView('results')}
      />
    </>
  );
});
