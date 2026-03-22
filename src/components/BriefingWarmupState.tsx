import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { useAppStore } from '../store';
import { useGameComponent } from '../hooks/use-game-component';

interface SourceInfo {
  type: string;
  name: string;
  enabled: boolean;
}

export function BriefingWarmupState({ onAnalyze }: { onAnalyze: () => void }) {
  const { t } = useTranslation();
  const userContext = useAppStore(s => s.userContext);
  const isBrowserMode = useAppStore(s => s.isBrowserMode);
  const fired = useRef(false);
  const [enabledSources, setEnabledSources] = useState<string[]>([]);
  const [autoStartPending, setAutoStartPending] = useState(!isBrowserMode);
  const { containerRef: turingRef } = useGameComponent('game-turing-fire');

  // Load actual configured sources from the backend
  useEffect(() => {
    cmd('get_sources')
      .then(r => r as unknown as SourceInfo[])
      .then(sources => {
        const enabled = sources
          .filter(s => s.enabled)
          .map(s => s.name);
        setEnabledSources(enabled.length > 0 ? enabled : ['Hacker News', 'Reddit', 'GitHub']);
      })
      .catch(() => {
        // Fallback: always-on sources if backend unavailable
        setEnabledSources(['Hacker News', 'Reddit', 'GitHub']);
      });
  }, []);

  // Auto-start analysis after 3 seconds so new users aren't stuck (skip in browser mode)
  useEffect(() => {
    if (fired.current || isBrowserMode) {
      setAutoStartPending(false);
      return;
    }
    const timer = setTimeout(() => {
      fired.current = true;
      setAutoStartPending(false);
      onAnalyze();
    }, 3000);
    return () => clearTimeout(timer);
  }, [onAnalyze, isBrowserMode]);

  // Gather detected info
  const stack = userContext?.tech_stack || [];

  return (
    <div className="relative text-center py-12 px-6">
      <div ref={turingRef} className="absolute inset-0 opacity-[0.15] rounded-lg overflow-hidden pointer-events-none" aria-hidden="true" />
      <div className="relative max-w-md mx-auto">
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

        {autoStartPending && (
          <p className="text-xs text-text-muted mt-3 animate-pulse">
            {t('briefing.warmup.autoStart', 'Starting automatically...')}
          </p>
        )}
      </div>
    </div>
  );
}
