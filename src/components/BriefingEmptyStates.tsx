// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { useLicense } from '../hooks/use-license';

/** Analysis in progress — spinner + live progress */
export function BriefingLoadingState() {
  const { t } = useTranslation();
  const results = useAppStore(s => s.appState.relevanceResults);
  const progress = useAppStore(s => s.appState.progress);
  const progressStage = useAppStore(s => s.appState.progressStage);
  const setActiveView = useAppStore(s => s.setActiveView);

  const stageLabel = progressStage === 'fetch' || progressStage === 'scrape'
    ? t('briefing.loadingStageFetch', 'Scanning sources for signals...')
    : progressStage === 'embed' || progressStage === 'relevance' || progressStage === 'rerank'
    ? t('briefing.loadingStageScore', 'Scoring items against your profile...')
    : t('briefing.loadingStageInit', 'Preparing analysis...');

  return (
    <div className="bg-bg-primary rounded-lg" role="status" aria-busy="true" aria-label={t('briefing.gatheringIntelligence')}>
      <div className="flex flex-col items-center justify-center py-20 px-8">
        <div className="w-20 h-20 mb-6 bg-orange-500/10 rounded-2xl border border-orange-500/20 flex items-center justify-center">
          <div className="w-6 h-6 border-2 border-orange-400 border-t-transparent rounded-full animate-spin" />
        </div>
        <h2 className="text-xl font-medium text-text-primary mb-2">{t('briefing.gatheringIntelligence')}</h2>
        <p className="text-sm text-text-secondary text-center max-w-md">
          {stageLabel}
        </p>
        {progress > 0 && (
          <div className="w-48 mt-4">
            <div className="w-full h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
              <div
                className="h-full bg-gradient-to-r from-orange-600 to-orange-400 transition-all duration-500 ease-out rounded-full"
                style={{ width: `${Math.max(progress * 100, 5)}%` }}
              />
            </div>
            <span className="text-xs text-text-muted mt-1 block text-center">{Math.round(progress * 100)}%</span>
          </div>
        )}
        {results.length > 0 && (
          <button onClick={() => setActiveView('results')} className="mt-6 text-sm text-text-muted hover:text-text-secondary transition-colors">
            {t('briefing.browseResults', { count: results.length })}
          </button>
        )}
      </div>
    </div>
  );
}

/** Analysis done, briefing available to generate */
export function BriefingReadyState() {
  const { t } = useTranslation();
  const results = useAppStore(s => s.appState.relevanceResults);
  const generateBriefing = useAppStore(s => s.generateBriefing);
  const startTrial = useAppStore(s => s.startTrial);
  const isLoading = useAppStore(s => s.aiBriefing.loading);
  const briefingError = useAppStore(s => s.aiBriefing.error);
  const { isPro, trialStatus } = useLicense();
  // Covers the gap between the click and the store flipping to loading; reset
  // when the request settles, so a failed generation can be retried here.
  const [requested, setRequested] = useState(false);
  const [startingTrial, setStartingTrial] = useState(false);
  const [trialFailed, setTrialFailed] = useState(false);

  const canStartTrial = !trialStatus?.started_at;

  const generate = async () => {
    setRequested(true);
    try {
      await generateBriefing();
    } finally {
      setRequested(false);
    }
  };

  const handleGenerate = () => {
    if (requested || isLoading) return;
    void generate();
  };

  const handleStartTrial = async () => {
    setStartingTrial(true);
    setTrialFailed(false);
    const ok = await startTrial();
    setStartingTrial(false);
    if (ok) {
      // Trial started — now generate immediately
      void generate();
    } else {
      setTrialFailed(true);
    }
  };

  const busy = requested || isLoading;

  return (
    <div className="bg-bg-primary rounded-lg">
      <div className="flex flex-col items-center justify-center py-20 px-8">
        <h2 className="text-xl font-medium text-text-primary mb-2">{t('briefing.readyToGenerate')}</h2>
        <p className="text-sm text-text-muted text-center max-w-md mb-6">
          {t('briefing.resultsAnalyzed', { count: results.length })}
        </p>
        {briefingError && !busy && (
          <p role="alert" className="text-xs text-red-400 text-center max-w-md mb-4">
            {t('briefing.generateFailed', { error: briefingError })}
          </p>
        )}
        {isPro ? (
          <button onClick={handleGenerate} disabled={busy} aria-label={t('briefing.generateAria')} className="px-6 py-2.5 bg-orange-500 text-white text-sm font-medium rounded-lg hover:bg-orange-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed">
            {busy ? (
              <span className="flex items-center gap-2">
                <span className="w-3.5 h-3.5 border-2 border-text-primary/30 border-t-text-primary rounded-full animate-spin" />
                {t('briefing.generate')}
              </span>
            ) : t('briefing.generate')}
          </button>
        ) : (
          <div className="flex flex-col items-center gap-3">
            <div className="flex items-center gap-2">
              <button onClick={handleGenerate} disabled={busy} aria-label={t('briefing.generateAria')} className="px-6 py-2.5 bg-orange-500 text-white text-sm font-medium rounded-lg hover:bg-orange-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed">
                {busy ? (
                  <span className="flex items-center gap-2">
                    <span className="w-3.5 h-3.5 border-2 border-text-primary/30 border-t-text-primary rounded-full animate-spin" />
                    {t('briefing.generate')}
                  </span>
                ) : t('briefing.generate')}
              </button>
              {canStartTrial && (
                <button
                  onClick={() => { void handleStartTrial(); }}
                  disabled={startingTrial}
                  className="px-5 py-2.5 text-sm font-medium text-accent-gold border border-accent-gold/30 rounded-lg hover:bg-accent-gold/10 transition-colors disabled:opacity-50"
                >
                  {startingTrial ? t('pro.startingTrial') : t('pro.startTrial')}
                </button>
              )}
            </div>
            {trialFailed && (
              <p role="alert" className="text-xs text-red-400">
                {t('briefing.trialStartFailed')}
              </p>
            )}
            <p className="text-xs text-text-muted mt-1">
              {t('briefing.signalFeatureNote', 'AI briefings are a Signal feature. Start a free trial to try it.')}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
