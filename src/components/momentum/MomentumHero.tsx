import { useRef, useEffect, useCallback, memo } from 'react';
import { registerGameComponent } from '../../lib/game-components';
import type { VoidSignal } from '../../types';
import type { CompoundAdvantageScore } from '../../types/autophagy';
import type { KnowledgeGap } from '../../types/innovation';
import type { RadarEntry } from '../tech-radar/RadarSVG';
import { MomentumGauges } from './MomentumGauges';

// ---------------------------------------------------------------------------
// Data Bridge — maps 4DA intelligence state to shader uniforms
// ---------------------------------------------------------------------------

function computeFieldParams(
  advantage: CompoundAdvantageScore | null,
  signal: VoidSignal,
  entries: RadarEntry[],
  gaps: KnowledgeGap[],
): Record<string, number> {
  const score = advantage !== null ? advantage.score / 100 : 0.3;
  const trend = advantage !== null ? advantage.trend : 0;
  const critGaps = gaps.filter(g => g.gap_severity === 'critical' || g.gap_severity === 'high').length;
  const coverage = entries.length > 0 ? 1 - (critGaps / entries.length) : 0.8;

  return {
    advantage: score,
    trend_norm: (trend + 1) / 2,
    trend_warm_r: trend > 0 ? 0.83 : 0.3,
    trend_warm_g: trend > 0 ? 0.65 : 0.4,
    metabolism: signal.metabolism,
    density: Math.min(entries.filter(e => e.movement !== 'stable').length / 15, 1),
    urgency: signal.signal_urgency,
    confidence: advantage !== null ? advantage.calibration_accuracy : 0.5,
    coverage,
  };
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface MomentumHeroProps {
  signal: VoidSignal;
  advantage: CompoundAdvantageScore | null;
  history: number[];
  entries: RadarEntry[];
  gaps: KnowledgeGap[];
}

export const MomentumHero = memo(function MomentumHero({
  signal, advantage, history, entries, gaps,
}: MomentumHeroProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const elementRef = useRef<HTMLElement | null>(null);

  // Mount the purpose-built momentum-field shader
  useEffect(() => {
    let cancelled = false;
    registerGameComponent('game-momentum-field').then(() => {
      if (cancelled || !containerRef.current) return;
      const el = document.createElement('game-momentum-field');
      el.style.width = '100%';
      el.style.height = '100%';
      el.style.display = 'block';
      el.style.borderRadius = '8px';
      containerRef.current.appendChild(el);
      elementRef.current = el;
    }).catch(() => { /* graceful — gauges work without shader */ });
    const container = containerRef.current;
    return () => {
      cancelled = true;
      if (elementRef.current && container?.contains(elementRef.current)) {
        container.removeChild(elementRef.current);
      }
      elementRef.current = null;
    };
  }, []);

  // Drive shader uniforms from real intelligence data
  const setParam = useCallback((name: string, value: number) => {
    const el = elementRef.current as (HTMLElement & { setParam?: (n: string, v: number) => void }) | null;
    el?.setParam?.(name, value);
  }, []);

  useEffect(() => {
    const params = computeFieldParams(advantage, signal, entries, gaps);
    for (const [key, val] of Object.entries(params)) {
      setParam(key, val);
    }
  }, [advantage, signal, entries, gaps, setParam]);

  return (
    <div className="space-y-3">
      {/* Momentum Field — ambient intelligence horizon */}
      <div
        ref={containerRef}
        className="w-full bg-[#0A0A0A] rounded-lg border border-border overflow-hidden"
        style={{ height: 80 }}
      />

      {/* Gauge Row — precise measurements */}
      <MomentumGauges
        advantage={advantage}
        history={history}
        entries={entries}
        gaps={gaps}
      />
    </div>
  );
});
