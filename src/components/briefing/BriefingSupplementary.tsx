import { memo, useCallback, useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import { useAppStore } from '../../store';
import { StreetsContextCard } from '../StreetsContextCard';
import { WeeklyIntelligenceSummary } from '../WeeklyIntelligenceSummary';
import { GuidedMissions } from '../GuidedMissions';
import { ContextualTip } from '../ContextualTip';
import IntelligenceReportCard from '../IntelligenceReport';
import type { SourceRelevance } from '../../types';

interface StreetsSuggestionData {
  module_id: string;
  module_title: string;
  reason: string;
  match_strength: number;
}

interface BriefingSupplementaryProps {
  results: SourceRelevance[];
  feedbackGiven: Record<number, string>;
  embeddingMode: string | null;
  analysisComplete: boolean;
}

/**
 * Supplementary briefing sections that appear below the main content panel:
 * STREETS suggestion, weekly summary, guided missions, contextual tips, and intelligence report.
 */
export const BriefingSupplementary = memo(function BriefingSupplementary({
  results,
  feedbackGiven,
  embeddingMode,
  analysisComplete,
}: BriefingSupplementaryProps) {
  const { t } = useTranslation();
  const setActiveView = useAppStore(s => s.setActiveView);

  // STREETS contextual suggestion
  const [streetsSuggestion, setStreetsSuggestion] = useState<StreetsSuggestionData | null>(null);

  useEffect(() => {
    cmd('get_streets_suggestion')
      .then((suggestion) => {
        if (!suggestion) {
          setStreetsSuggestion(null);
          return;
        }
        const dismissKey = `streets_dismiss_${suggestion.module_id}`;
        const dismissedAt = localStorage.getItem(dismissKey);
        if (dismissedAt) {
          const elapsed = Date.now() - parseInt(dismissedAt, 10);
          if (elapsed < 7 * 24 * 60 * 60 * 1000) {
            setStreetsSuggestion(null);
            return;
          }
          localStorage.removeItem(dismissKey);
        }
        setStreetsSuggestion(suggestion);
      })
      .catch(() => setStreetsSuggestion(null));
  }, [analysisComplete]);

  const handleStreetsDismiss = useCallback((moduleId: string) => {
    localStorage.setItem(`streets_dismiss_${moduleId}`, Date.now().toString());
    setStreetsSuggestion(null);
  }, []);

  const handleStreetsOpen = useCallback((moduleId: string) => {
    setActiveView('playbook');
    setTimeout(() => {
      const store = useAppStore.getState();
      store.loadPlaybookContent?.(moduleId);
    }, 100);
  }, [setActiveView]);

  return (
    <>
      {/* STREETS Contextual Suggestion */}
      {streetsSuggestion && (
        <StreetsContextCard
          suggestion={streetsSuggestion}
          onOpen={handleStreetsOpen}
          onDismiss={handleStreetsDismiss}
        />
      )}

      {/* Weekly intelligence summary */}
      <WeeklyIntelligenceSummary />

      {/* Guided missions — first 48h onboarding */}
      <GuidedMissions />

      {/* Contextual tip: teach feedback loop */}
      <ContextualTip
        tipId="feedback-loop"
        message={t('tips.feedbackLoop', 'Save articles you find useful — this teaches the system what matters to you.')}
        hint={t('tips.feedbackLoopHint', 'Dismissing articles also helps. Every interaction improves your results.')}
        showWhen={Object.keys(feedbackGiven).length === 0 && results.length > 0}
      />

      {/* Contextual tip: Ollama nudge for keyword-only users */}
      <ContextualTip
        tipId="ollama-nudge"
        message={t('tips.ollamaNudge', 'Your results use keyword matching. Ollama (free, local) unlocks semantic matching for more relevant results.')}
        hint={t('tips.ollamaNudgeHint', 'Install Ollama, then enable it in Settings > AI Provider. Runs entirely on your machine.')}
        showWhen={embeddingMode === 'keyword-only' && results.length > 0}
      />

      {/* Intelligence Report */}
      <IntelligenceReportCard />
    </>
  );
});
