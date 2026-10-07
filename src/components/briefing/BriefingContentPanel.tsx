// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useEffect } from 'react';
import { NarratedBrief } from './NarratedBrief';
import { IntelligenceFeed } from './IntelligenceFeed';
import { useTranslatedContent } from '../ContentTranslationProvider';
import type { SourceRelevance, FeedbackAction } from '../../types';
import type { BriefingState } from '../../store/types';

import type { ActiveView } from '../../store/types';

interface BriefingContentPanelProps {
  briefing: BriefingState;
  results: SourceRelevance[];
  feedbackGiven: Record<number, FeedbackAction>;
  onSave: (item: SourceRelevance) => void;
  onDismiss: (item: SourceRelevance) => void;
  onRecordClick: (item: SourceRelevance) => void;
  onUnsave?: (item: SourceRelevance) => void;
  onRegenerate: () => void;
  setActiveView: (view: ActiveView) => void;
}

const NO_SIGNAL_IDS: Set<number> = new Set<number>();

/**
 * The Brief tab: the brief itself, then the review queue.
 *
 * Zone 1 (Brief): what changed that touches your code — written from facts
 * 4DA computed (confirmed security with its real fix path, breaking upgrades
 * of direct dependencies, fresh judge-approved reading), never from keyword
 * classification (Decision 2, 2026-10-02).
 * Zone 2 (Queue): the ranked feed, for reading beyond the brief.
 */
export const BriefingContentPanel = memo(function BriefingContentPanel({
  briefing,
  results,
  feedbackGiven,
  onSave,
  onDismiss,
  onRecordClick,
  onUnsave,
  onRegenerate,
  setActiveView,
}: BriefingContentPanelProps) {
  // Request content translation for the queue's visible titles.
  const { requestTranslation } = useTranslatedContent();
  useEffect(() => {
    if (results.length === 0) return;
    requestTranslation(results.map(item => ({ id: String(item.id), text: item.title })));
  }, [results, requestTranslation]);

  return (
    <>
      {briefing.content && (
        <NarratedBrief
          content={briefing.content}
          model={briefing.model}
          lastGenerated={briefing.lastGenerated}
          loading={briefing.loading}
          onRegenerate={onRegenerate}
        />
      )}

      <IntelligenceFeed
        results={results}
        feedbackGiven={feedbackGiven}
        signalIds={NO_SIGNAL_IDS}
        onSave={onSave}
        onDismiss={onDismiss}
        onRecordClick={onRecordClick}
        onUnsave={onUnsave}
        onViewAll={() => setActiveView('results')}
      />
    </>
  );
});
