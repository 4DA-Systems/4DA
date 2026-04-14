import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { useLicense } from '../hooks/use-license';
import { useGameComponent } from '../hooks/use-game-component';
import { registerGameComponent } from '../lib/game-components';

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
    <div className="bg-bg-primary rounded-lg" role="status" aria-busy="true" aria-label="Gathering intelligence">
      <div className="flex flex-col items-center justify-center py-20 px-8">
        <div className="w-20 h-20 mb-6 bg-orange-500/10 rounded-2xl border border-orange-500/20 flex items-center justify-center">
          <div className="w-6 h-6 border-2 border-orange-400 border-t-transparent rounded-full animate-spin" />
        </div>
        <h2 className="text-xl font-medium text-white mb-2">{t('briefing.gatheringIntelligence')}</h2>
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
  const { isPro, trialStatus } = useLicense();
  const [clicked, setClicked] = useState(false);
  const [startingTrial, setStartingTrial] = useState(false);

  const canStartTrial = !trialStatus?.started_at;

  const handleGenerate = () => {
    if (clicked || isLoading) return;
    setClicked(true);
    generateBriefing();
  };

  const handleStartTrial = async () => {
    setStartingTrial(true);
    const ok = await startTrial();
    setStartingTrial(false);
    if (ok) {
      // Trial started — now generate immediately
      setClicked(true);
      generateBriefing();
    }
  };

  const busy = clicked || isLoading;

  return (
    <div className="bg-bg-primary rounded-lg">
      <div className="flex flex-col items-center justify-center py-20 px-8">
        <div className="w-20 h-20 mb-6 bg-bg-secondary rounded-2xl border border-orange-500/20 flex items-center justify-center">
          <svg className="w-8 h-8 text-orange-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z" />
          </svg>
        </div>
        <h2 className="text-xl font-medium text-white mb-2">{t('briefing.readyToGenerate')}</h2>
        <p className="text-sm text-text-muted text-center max-w-md mb-6">
          {t('briefing.resultsAnalyzed', { count: results.length })}
        </p>
        {isPro ? (
          <button onClick={handleGenerate} disabled={busy} aria-label="Generate intelligence briefing" className="px-6 py-2.5 bg-orange-500 text-white text-sm font-medium rounded-lg hover:bg-orange-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed">
            {busy ? (
              <span className="flex items-center gap-2">
                <span className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                {t('briefing.generate')}
              </span>
            ) : t('briefing.generate')}
          </button>
        ) : (
          <div className="flex flex-col items-center gap-3">
            <div className="flex items-center gap-2">
              <button onClick={handleGenerate} disabled={busy} aria-label="Generate intelligence briefing" className="px-6 py-2.5 bg-orange-500 text-white text-sm font-medium rounded-lg hover:bg-orange-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed">
                {busy ? (
                  <span className="flex items-center gap-2">
                    <span className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                    {t('briefing.generate')}
                  </span>
                ) : t('briefing.generate')}
              </button>
              {canStartTrial && (
                <button
                  onClick={handleStartTrial}
                  disabled={startingTrial}
                  className="px-5 py-2.5 text-sm font-medium text-accent-gold border border-accent-gold/30 rounded-lg hover:bg-accent-gold/10 transition-colors disabled:opacity-50"
                >
                  {startingTrial ? t('pro.startingTrial') : t('pro.startTrial')}
                </button>
              )}
            </div>
            <p className="text-xs text-text-muted mt-1">
              {t('briefing.signalFeatureNote', 'AI briefings are a Signal feature. Start a free trial to try it.')}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

/** No analysis yet — "Analyze Now" CTA */
export function BriefingNoDataState() {
  const { t } = useTranslation();
  const startAnalysis = useAppStore(s => s.startAnalysis);
  const setShowSettings = useAppStore(s => s.setShowSettings);
  const embeddingMode = useAppStore(s => s.embeddingMode);
  const { containerRef: turingRef } = useGameComponent('fourda-turing-fire');

  useEffect(() => { registerGameComponent('fourda-simplex-unfold'); }, []);

  return (
    <div className="relative bg-bg-primary rounded-lg">
      <div ref={turingRef} className="absolute inset-0 opacity-[0.18] rounded-lg overflow-hidden pointer-events-none" aria-hidden="true" />
      <div className="relative flex flex-col items-center justify-center py-20 px-8">
        <div className="w-[120px] h-[120px] mb-6 rounded-2xl border border-border/30 overflow-hidden" role="img" aria-label="4DA">
          <fourda-simplex-unfold style={{ width: '120px', height: '120px', display: 'block' }} />
        </div>
        <h2 className="text-xl font-medium text-white mb-2">{t('briefing.noIntelligence')}</h2>
        <p className="text-sm text-text-muted text-center max-w-md mb-6">
          {t('briefing.runAnalysis')}
        </p>
        <button onClick={startAnalysis} aria-label="Start content analysis" className="px-6 py-2.5 bg-orange-500 text-white text-sm font-medium rounded-lg hover:bg-orange-600 transition-colors">
          {t('results.analyzeNow')}
        </button>
        <p className="text-xs text-text-muted mt-3">
          {t('briefing.orPress')} <kbd className="px-1.5 py-0.5 bg-bg-tertiary rounded text-text-muted">R</kbd>
        </p>
        {embeddingMode === 'keyword-only' && (
          <button
            onClick={() => setShowSettings(true)}
            className="text-xs text-amber-400 hover:text-amber-300 transition-colors mt-3"
          >
            {t('briefing.configureAiHint', 'Keyword mode active — configure AI provider in Settings for better results')}
          </button>
        )}
      </div>
    </div>
  );
}
