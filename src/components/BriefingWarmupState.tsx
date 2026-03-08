import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

export function BriefingWarmupState({ onAnalyze }: { onAnalyze: () => void }) {
  const { t } = useTranslation();
  const userContext = useAppStore(s => s.userContext);
  const fired = useRef(false);

  // Auto-start analysis after 3 seconds so new users aren't stuck
  useEffect(() => {
    if (fired.current) return;
    const timer = setTimeout(() => {
      fired.current = true;
      onAnalyze();
    }, 3000);
    return () => clearTimeout(timer);
  }, [onAnalyze]);

  // Gather detected info
  const stack = userContext?.tech_stack || [];
  const sources = ['Hacker News', 'Reddit', 'GitHub', 'arXiv', 'RSS'];
  const enabledSources = sources; // Could filter by settings

  return (
    <div className="text-center py-12 px-6">
      <div className="max-w-md mx-auto">
        <h2 className="text-xl font-semibold text-white mb-2">
          {t('briefing.warmup.title', 'Your Intelligence System')}
        </h2>

        {stack.length > 0 && (
          <div className="mb-4">
            <p className="text-text-secondary text-sm mb-2">
              {t('briefing.warmup.stackDetected', 'Stack detected')}
            </p>
            <div className="flex flex-wrap gap-1.5 justify-center">
              {stack.slice(0, 8).map(tech => (
                <span key={tech} className="px-2 py-0.5 bg-white/10 text-white text-xs rounded">
                  {tech}
                </span>
              ))}
            </div>
          </div>
        )}

        <div className="mb-6">
          <p className="text-text-secondary text-sm mb-2">
            {t('briefing.warmup.sourcesReady', 'Sources ready')}
          </p>
          <div className="flex flex-wrap gap-1.5 justify-center">
            {enabledSources.map(source => (
              <span key={source} className="px-2 py-0.5 bg-accent-gold/10 text-accent-gold text-xs rounded">
                {source}
              </span>
            ))}
          </div>
        </div>

        <p className="text-text-muted text-sm mb-6">
          {t('briefing.warmup.description', '4DA will scan sources, score every item against your profile, and surface what matters.')}
        </p>

        <button
          onClick={onAnalyze}
          className="px-6 py-2.5 bg-white text-black font-medium rounded-lg hover:bg-gray-200 transition-colors"
        >
          {t('briefing.warmup.activate', 'Start Intelligence')}
        </button>

        <p className="text-xs text-text-muted mt-3 animate-pulse">
          {t('briefing.warmup.autoStart', 'Starting automatically...')}
        </p>
      </div>
    </div>
  );
}
