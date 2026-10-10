// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { isVictauriDogfoodMode } from '../lib/startup-runtime';
import { useAppStore } from '../store';
import { AmbientGlow } from './AmbientGlow';
import { ColdStartCard } from './ColdStartCard';

export function BriefingWarmupState({ onAnalyze }: { onAnalyze: () => void }) {
  const { t } = useTranslation();
  const userContext = useAppStore(s => s.userContext);
  const isBrowserMode = useAppStore(s => s.isBrowserMode);
  const analysisComplete = useAppStore(s => s.appState.analysisComplete);
  const fired = useRef(false);
  const [enabledSources, setEnabledSources] = useState<string[]>([]);
  // null until the source list loads; interests are opt-in (AD-054).
  const [interestsOn, setInterestsOn] = useState<number | null>(null);
  const [autoStartPending, setAutoStartPending] = useState(!isBrowserMode);

  // Load actual configured sources from the backend
  useEffect(() => {
    void cmd('get_sources')
      .then(rawSources => {
        const sources = Array.isArray(rawSources) ? rawSources : [];
        const enabled = sources.filter(s => s.enabled);
        // What is actually on — never a placeholder list of sources that
        // are not (interests are off until the user turns them on).
        setEnabledSources(enabled.map(s => s.name));
        setInterestsOn(enabled.filter(s => s.class === 'interest').length);
      })
      .catch(() => {
        setEnabledSources([]);
      });
  }, []);

  // Auto-start analysis after 3 seconds so new users aren't stuck (skip in browser mode)
  // Cooldown prevents hot-reload restart loops from re-triggering analysis endlessly
  useEffect(() => {
    if (fired.current || isBrowserMode) {
      setAutoStartPending(false);
      return;
    }
    const lastAuto = Number(window.sessionStorage.getItem('4da-last-auto-analysis') ?? '0');
    if (Date.now() - lastAuto < 15_000) {
      setAutoStartPending(false);
      return;
    }
    let timer: ReturnType<typeof setTimeout> | undefined;
    let cancelled = false;
    void isVictauriDogfoodMode().then((dogfoodMode) => {
      if (cancelled) return;
      if (dogfoodMode) {
        setAutoStartPending(false);
        return;
      }
      timer = setTimeout(() => {
        fired.current = true;
        setAutoStartPending(false);
        window.sessionStorage.setItem('4da-last-auto-analysis', String(Date.now()));
        onAnalyze();
      }, 3000);
    });
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [onAnalyze, isBrowserMode]);

  // Gather detected info
  const stack = userContext?.tech_stack || [];

  // Cold start (doctrine rule 6): no project found and no interest on, and the
  // first run has finished — a setup step, not an empty result.
  if (analysisComplete && stack.length === 0 && interestsOn === 0) {
    return <ColdStartCard />;
  }

  return (
    <div className="relative text-center py-12 px-6">
      <AmbientGlow />
      <div className="relative max-w-md mx-auto">
        <h2 className="text-xl font-semibold text-text-primary mb-2">
          {t('briefing.warmup.title', 'Your Intelligence System')}
        </h2>

        {stack.length > 0 && (
          <div className="mb-4">
            <p className="text-text-secondary text-sm mb-2">
              {t('briefing.warmup.stackDetected', 'Stack detected')}
            </p>
            <div className="flex flex-wrap gap-1.5 justify-center">
              {stack.slice(0, 8).map(tech => (
                <span key={tech} className="px-2 py-0.5 bg-text-primary/10 text-text-primary text-xs rounded">
                  {tech}
                </span>
              ))}
            </div>
          </div>
        )}

        {enabledSources.length > 0 && (
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
        )}

        <p className="text-text-muted text-sm mb-6">
          {t('briefing.warmup.description', '4DA will scan sources, score every item against your profile, and surface what matters.')}
        </p>

        <button
          onClick={onAnalyze}
          className="px-6 py-2.5 bg-accent-primary text-bg-primary font-medium rounded-lg hover:bg-accent-primary-hover transition-colors"
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
