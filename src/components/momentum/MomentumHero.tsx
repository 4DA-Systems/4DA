import { useRef, useEffect, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { registerGameComponent } from '../../lib/game-components';
import type { VoidSignal } from '../../types';
import type { CompoundAdvantageScore } from '../../types/autophagy';

// ---------------------------------------------------------------------------
// Sparkline (inline — keeps component self-contained)
// ---------------------------------------------------------------------------

function Sparkline({ data }: { data: number[] }) {
  if (data.length < 2) return null;
  const max = Math.max(...data, 1);
  const min = Math.min(...data, 0);
  const range = max - min || 1;
  const w = 64, h = 20;
  const pts = data.map((v, i) => `${(i / (data.length - 1)) * w},${h - ((v - min) / range) * h}`).join(' ');
  return (
    <svg width={w} height={h} className="inline-block opacity-60">
      <polyline points={pts} fill="none" stroke="currentColor" strokeWidth="1.5" className="text-accent-gold" />
    </svg>
  );
}

function TrendArrow({ trend }: { trend: number }) {
  if (trend > 0.05) return <span className="text-green-400 text-sm">{'\u2191'}</span>;
  if (trend < -0.05) return <span className="text-red-400 text-sm">{'\u2193'}</span>;
  return <span className="text-text-muted text-sm">{'\u2192'}</span>;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface MomentumHeroProps {
  signal: VoidSignal;
  advantage: CompoundAdvantageScore | null;
  history: number[];
}

export const MomentumHero = memo(function MomentumHero({ signal, advantage, history }: MomentumHeroProps) {
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement>(null);
  const elementRef = useRef<HTMLElement | null>(null);

  // Mount GAME shader
  useEffect(() => {
    let cancelled = false;
    registerGameComponent('game-signal-waveform').then(() => {
      if (cancelled || !containerRef.current) return;
      const el = document.createElement('game-signal-waveform');
      el.style.width = '100%';
      el.style.height = '100%';
      el.style.display = 'block';
      el.style.borderRadius = '8px';
      containerRef.current.appendChild(el);
      elementRef.current = el;
    }).catch(() => { /* graceful — hero works without shader */ });
    const container = containerRef.current;
    return () => {
      cancelled = true;
      if (elementRef.current && container?.contains(elementRef.current)) {
        container.removeChild(elementRef.current);
      }
      elementRef.current = null;
    };
  }, []);

  // Drive shader from VoidSignal
  const setParam = useCallback((name: string, value: number) => {
    const el = elementRef.current as (HTMLElement & { setParam?: (n: string, v: number) => void }) | null;
    el?.setParam?.(name, value);
  }, []);

  useEffect(() => {
    setParam('intensity', signal.signal_intensity);
    setParam('pulse', signal.pulse);
    setParam('heat', signal.heat);
    setParam('color_shift', signal.advantage_trend > 0 ? 0.6 : signal.advantage_trend < 0 ? -0.4 : 0);
    setParam('metabolism', signal.metabolism);
  }, [signal, setParam]);

  const score = advantage ? Math.round(advantage.score) : null;
  const scoreColor = score !== null && score >= 60 ? 'text-green-400'
    : score !== null && score >= 30 ? 'text-accent-gold'
    : 'text-text-secondary';

  const sparkData = history.length >= 2
    ? history
    : advantage ? [Math.max(advantage.score - 8, 0), advantage.score - 3, advantage.score] : [];

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden flex" style={{ height: 120 }}>
      {/* Shader waveform */}
      <div ref={containerRef} className="flex-1 min-w-0 bg-[#0D0D0D]" />

      {/* Compound Advantage */}
      <div className="w-48 flex flex-col items-center justify-center px-4 border-s border-border gap-1">
        {score !== null ? (
          <>
            <div className="flex items-center gap-1.5">
              <span className={`text-2xl font-bold tabular-nums ${scoreColor}`}>{score}</span>
              <TrendArrow trend={advantage!.trend} />
            </div>
            <Sparkline data={sparkData} />
            <span className="text-[9px] text-text-muted uppercase tracking-widest">
              {t('momentum.compoundAdvantage')}
            </span>
          </>
        ) : (
          <span className="text-[10px] text-text-muted">{t('momentum.loading')}</span>
        )}
      </div>
    </div>
  );
});
