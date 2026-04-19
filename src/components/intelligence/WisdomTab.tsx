// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState, useEffect, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { WisdomPanel } from '../WisdomPanel';
import type { AweWellItem, AwePendingDecision, AweBehavioralContext } from '../../types/awe';

// ============================================================================
// Constants
// ============================================================================

const WELL_LAYERS: Array<{ key: keyof Omit<import('../../types/awe').AweWisdomWell, never>; label: string; color: string }> = [
  { key: 'surface',   label: 'Surface',   color: 'bg-blue-400' },
  { key: 'pattern',   label: 'Pattern',   color: 'bg-cyan-400' },
  { key: 'principle', label: 'Principle', color: 'bg-accent-gold' },
  { key: 'causal',    label: 'Causal',    color: 'bg-amber-500' },
  { key: 'meta',      label: 'Meta',      color: 'bg-purple-400' },
  { key: 'universal', label: 'Universal', color: 'bg-success' },
];

// ============================================================================
// Sub-components
// ============================================================================

function SectionToggle({
  title,
  open,
  onToggle,
  children,
}: {
  title: string;
  open: boolean;
  onToggle: () => void;
  children: React.ReactNode;
}) {
  return (
    <div className="border-b border-border/30 last:border-b-0">
      <button
        onClick={onToggle}
        aria-expanded={open}
        className="w-full flex items-center justify-between px-4 py-3 hover:bg-bg-tertiary/50 transition-colors"
      >
        <span className="text-xs font-medium text-text-primary">{title}</span>
        <span className="text-text-muted text-xs" aria-hidden="true">{open ? '\u25B2' : '\u25BC'}</span>
      </button>
      {open && <div className="px-4 pb-4">{children}</div>}
    </div>
  );
}

function WellLayer({ label, color, items }: { label: string; color: string; items: AweWellItem[] }) {
  const count = items.length;
  const maxWidth = Math.min(100, Math.max(8, count * 15));

  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between">
        <span className="text-[10px] text-text-muted font-mono uppercase">{label}</span>
        <span className="text-[10px] text-text-muted tabular-nums">{count}</span>
      </div>
      <div className="h-3 bg-bg-primary rounded overflow-hidden">
        <div
          className={`h-full rounded transition-all ${color}`}
          style={{ width: `${maxWidth}%`, opacity: count > 0 ? 1 : 0.2 }}
        />
      </div>
      {items.length > 0 && (
        <ul className="mt-1 space-y-0.5">
          {items.slice(0, 3).map((item, i) => (
            <li key={i} className="text-[10px] text-text-muted font-mono truncate">
              {'\u25C6'} {item.statement}
              <span className="text-text-muted/50 ml-1">({Math.round(item.confidence * 100)}%)</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function CalibrationStats() {
  const { t } = useTranslation();
  const aweSummary = useAppStore(s => s.aweSummary);

  if (!aweSummary || !aweSummary.available) {
    return (
      <p className="text-xs text-text-muted italic">{t('awe.console.calibrationUnavailable')}</p>
    );
  }

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-3">
        <div className="bg-bg-tertiary rounded px-3 py-2">
          <div className="text-sm font-semibold text-white tabular-nums">{aweSummary.decisions}</div>
          <div className="text-[10px] text-text-muted">{t('awe.console.totalDecisions')}</div>
        </div>
        <div className="bg-bg-tertiary rounded px-3 py-2">
          <div className="text-sm font-semibold text-white tabular-nums">{aweSummary.principles}</div>
          <div className="text-[10px] text-text-muted">{t('awe.console.totalPrinciples')}</div>
        </div>
        <div className="bg-bg-tertiary rounded px-3 py-2">
          <div className="text-sm font-semibold text-white tabular-nums">{aweSummary.feedback_count}</div>
          <div className="text-[10px] text-text-muted">{t('awe.console.feedbackGiven')}</div>
        </div>
        <div className="bg-bg-tertiary rounded px-3 py-2">
          <div className="text-sm font-semibold text-white tabular-nums">
            {aweSummary.feedback_coverage > 0 ? `${aweSummary.feedback_coverage}%` : '--'}
          </div>
          <div className="text-[10px] text-text-muted">{t('awe.console.coverageRate')}</div>
        </div>
      </div>
      {aweSummary.health && (
        <div className="text-[10px] text-text-muted">
          {t('awe.console.healthStatus')}: <span className="text-text-secondary capitalize">{aweSummary.health.replace('_', ' ')}</span>
        </div>
      )}
    </div>
  );
}

function FeedbackQueueItem({
  decision,
  onConfirm,
  onRefute,
}: {
  decision: AwePendingDecision;
  onConfirm: () => void;
  onRefute: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex items-start justify-between gap-3 py-2 border-b border-border/20 last:border-b-0">
      <div className="flex-1 min-w-0">
        <p className="text-xs text-text-secondary truncate">{decision.statement}</p>
        <p className="text-[10px] text-text-muted mt-0.5">
          {decision.domain} &middot; {decision.age_days}d ago
        </p>
      </div>
      <div className="flex gap-1 flex-shrink-0">
        <button
          onClick={onConfirm}
          aria-label={`${t('awe.console.confirm')} ${decision.statement}`}
          className="text-[10px] px-2 py-0.5 rounded border border-success/30 text-success hover:bg-success/10 transition-colors"
        >
          {t('awe.console.confirm')}
        </button>
        <button
          onClick={onRefute}
          aria-label={`${t('awe.console.refute')} ${decision.statement}`}
          className="text-[10px] px-2 py-0.5 rounded border border-error/30 text-error hover:bg-error/10 transition-colors"
        >
          {t('awe.console.refute')}
        </button>
      </div>
    </div>
  );
}

function BehavioralInsights({ ctx }: { ctx: AweBehavioralContext }) {
  const ip = ctx.interaction_patterns;
  const velocity = ip.weekly_velocity > 1.5 ? 'Accelerating' : ip.weekly_velocity > 0.8 ? 'Steady' : ip.weekly_velocity > 0 ? 'Declining' : 'Starting';
  const velocityColor = ip.weekly_velocity > 1.5 ? 'text-success' : ip.weekly_velocity > 0.8 ? 'text-text-secondary' : 'text-amber-400';

  return (
    <div className="space-y-3">
      {/* Interaction Stats */}
      <div className="grid grid-cols-3 gap-2">
        <div className="bg-bg-tertiary rounded px-2 py-1.5 text-center">
          <div className="text-sm font-semibold text-white tabular-nums">{ip.total_interactions}</div>
          <div className="text-[9px] text-text-muted">Interactions</div>
        </div>
        <div className="bg-bg-tertiary rounded px-2 py-1.5 text-center">
          <div className="text-sm font-semibold text-white tabular-nums">{ip.saves}</div>
          <div className="text-[9px] text-text-muted">Saves</div>
        </div>
        <div className="bg-bg-tertiary rounded px-2 py-1.5 text-center">
          <div className={`text-sm font-semibold tabular-nums ${velocityColor}`}>{velocity}</div>
          <div className="text-[9px] text-text-muted">Velocity ({ip.weekly_velocity.toFixed(1)}x)</div>
        </div>
      </div>

      {/* Top Affinities */}
      {ctx.topic_affinities.length > 0 && (
        <div>
          <div className="text-[10px] text-text-muted uppercase tracking-wider mb-1.5">Topic Affinities</div>
          <div className="space-y-1">
            {ctx.topic_affinities.slice(0, 8).map(t => (
              <div key={t.topic} className="flex items-center gap-2">
                <span className="text-[10px] text-text-secondary truncate flex-1 font-mono">{t.topic}</span>
                <div className="w-16 h-1.5 bg-bg-primary rounded overflow-hidden">
                  <div
                    className={`h-full rounded ${t.affinity_score > 0 ? 'bg-success' : 'bg-error'}`}
                    style={{ width: `${Math.abs(t.affinity_score) * 100}%` }}
                  />
                </div>
                <span className="text-[9px] text-text-muted tabular-nums w-8 text-right">
                  {Math.round(t.affinity_score * 100)}%
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Top Sources */}
      {ip.top_sources.length > 0 && (
        <div>
          <div className="text-[10px] text-text-muted uppercase tracking-wider mb-1.5">Top Sources</div>
          <div className="flex flex-wrap gap-1.5">
            {ip.top_sources.map(([source, count]) => (
              <span key={source} className="text-[10px] text-text-secondary bg-bg-tertiary rounded px-2 py-0.5">
                {source}: {count}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Feedback Coverage */}
      <div className="flex items-center justify-between text-[10px]">
        <span className="text-text-muted">Feedback Coverage</span>
        <span className="text-text-secondary tabular-nums">{ctx.feedback_stats.coverage_pct.toFixed(1)}%</span>
      </div>

      {/* Advantage Score */}
      {ctx.advantage_trajectory.length > 0 && (
        <div className="flex items-center justify-between text-[10px]">
          <span className="text-text-muted">Compound Advantage</span>
          <span className="text-accent-gold tabular-nums">{ctx.advantage_trajectory[0]?.score.toFixed(1)}</span>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Main Component
// ============================================================================

/**
 * WisdomTab — Full AWE console with 4 collapsible sections:
 * 1. Transmutation (existing WisdomPanel)
 * 2. Wisdom Well (geological depth visualization)
 * 3. Calibration (summary metrics)
 * 4. Feedback Queue (pending decisions with confirm/refute)
 */
export const WisdomTab = memo(function WisdomTab() {
  const { t } = useTranslation();

  // Section open state
  const [openSections, setOpenSections] = useState<Record<string, boolean>>({
    behavioral: true,
    transmutation: false,
    well: false,
    calibration: false,
    feedback: false,
  });

  // Store bindings
  const aweSummary = useAppStore(s => s.aweSummary);
  const aweWisdomWell = useAppStore(s => s.aweWisdomWell);
  const awePendingDecisions = useAppStore(s => s.awePendingDecisions);
  const aweBehavioralContext = useAppStore(s => s.aweBehavioralContext);
  const aweWisdomSynthesis = useAppStore(s => s.aweWisdomSynthesis);
  const loadAweSummary = useAppStore(s => s.loadAweSummary);
  const loadAweWisdomWell = useAppStore(s => s.loadAweWisdomWell);
  const loadAwePendingDecisions = useAppStore(s => s.loadAwePendingDecisions);
  const loadBehavioralContext = useAppStore(s => s.loadBehavioralContext);
  const submitAweBatchFeedback = useAppStore(s => s.submitAweBatchFeedback);

  useEffect(() => {
    void loadAweSummary();
    void loadAweWisdomWell();
    void loadAwePendingDecisions();
    void loadBehavioralContext();
  }, [loadAweSummary, loadAweWisdomWell, loadAwePendingDecisions, loadBehavioralContext]);

  const toggle = useCallback((section: string) => {
    setOpenSections(prev => ({ ...prev, [section]: !prev[section] }));
  }, []);

  const handleFeedback = useCallback((decisionId: string, outcome: 'confirmed' | 'refuted') => {
    void submitAweBatchFeedback([{
      decision_id: decisionId,
      outcome,
      details: `Batch ${outcome} from console`,
    }]);
  }, [submitAweBatchFeedback]);

  // Don't render if AWE is not available
  if (aweSummary && !aweSummary.available) {
    return (
      <div className="p-5 text-center">
        <p className="text-xs text-text-muted">{t('awe.console.unavailable')}</p>
      </div>
    );
  }

  return (
    <div className="divide-y divide-border/30" role="region" aria-label={t('awe.console.title', 'Wisdom Console')}>
      {/* AWE Wisdom Synthesis Voice */}
      {aweWisdomSynthesis && (
        <div className="px-4 py-3 bg-accent-gold/5 border-b border-accent-gold/20">
          <p className="text-xs text-accent-gold/90 leading-relaxed">{aweWisdomSynthesis}</p>
        </div>
      )}

      {/* Section 0: Behavioral Intelligence (real 4DA data) */}
      <SectionToggle
        title={t('awe.console.behavioral', 'Behavioral Intelligence')}
        open={!!openSections.behavioral}
        onToggle={() => toggle('behavioral')}
      >
        {aweBehavioralContext ? (
          <BehavioralInsights ctx={aweBehavioralContext} />
        ) : (
          <p className="text-xs text-text-muted italic">{t('awe.console.behavioralLoading', 'Loading behavioral data...')}</p>
        )}
      </SectionToggle>

      {/* Section 1: Transmutation */}
      <SectionToggle
        title={t('awe.console.transmutation')}
        open={!!openSections.transmutation}
        onToggle={() => toggle('transmutation')}
      >
        <WisdomPanel />
      </SectionToggle>

      {/* Section 2: Wisdom Well */}
      <SectionToggle
        title={t('awe.console.wisdomWell')}
        open={!!openSections.well}
        onToggle={() => toggle('well')}
      >
        {aweWisdomWell ? (
          <div className="space-y-3">
            {WELL_LAYERS.map(layer => (
              <WellLayer
                key={layer.key}
                label={layer.label}
                color={layer.color}
                items={aweWisdomWell[layer.key] ?? []}
              />
            ))}
          </div>
        ) : (
          <p className="text-xs text-text-muted italic">{t('awe.console.wellEmpty')}</p>
        )}
      </SectionToggle>

      {/* Section 3: Calibration */}
      <SectionToggle
        title={t('awe.console.calibration')}
        open={!!openSections.calibration}
        onToggle={() => toggle('calibration')}
      >
        <CalibrationStats />
      </SectionToggle>

      {/* Section 4: Feedback Queue */}
      <SectionToggle
        title={
          awePendingDecisions.length > 0
            ? `${t('awe.console.feedbackQueue')} (${awePendingDecisions.length})`
            : t('awe.console.feedbackQueue')
        }
        open={!!openSections.feedback}
        onToggle={() => toggle('feedback')}
      >
        {awePendingDecisions.length > 0 ? (
          <div className="space-y-0">
            {awePendingDecisions.map(d => (
              <FeedbackQueueItem
                key={d.id}
                decision={d}
                onConfirm={() => handleFeedback(d.id, 'confirmed')}
                onRefute={() => handleFeedback(d.id, 'refuted')}
              />
            ))}
          </div>
        ) : (
          <p className="text-xs text-text-muted italic">{t('awe.console.queueEmpty')}</p>
        )}
      </SectionToggle>
    </div>
  );
});
