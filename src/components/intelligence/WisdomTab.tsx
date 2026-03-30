// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState, useEffect, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { WisdomPanel } from '../WisdomPanel';
import type { AweWellItem, AwePendingDecision } from '../../types/awe';

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
        className="w-full flex items-center justify-between px-4 py-3 hover:bg-bg-tertiary/50 transition-colors"
      >
        <span className="text-xs font-medium text-text-primary">{title}</span>
        <span className="text-text-muted text-xs">{open ? '\u25B2' : '\u25BC'}</span>
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
          className="text-[10px] px-2 py-0.5 rounded border border-success/30 text-success hover:bg-success/10 transition-colors"
        >
          {t('awe.console.confirm')}
        </button>
        <button
          onClick={onRefute}
          className="text-[10px] px-2 py-0.5 rounded border border-error/30 text-error hover:bg-error/10 transition-colors"
        >
          {t('awe.console.refute')}
        </button>
      </div>
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
    transmutation: true,
    well: false,
    calibration: false,
    feedback: false,
  });

  // Store bindings
  const aweSummary = useAppStore(s => s.aweSummary);
  const aweWisdomWell = useAppStore(s => s.aweWisdomWell);
  const awePendingDecisions = useAppStore(s => s.awePendingDecisions);
  const loadAweSummary = useAppStore(s => s.loadAweSummary);
  const loadAweWisdomWell = useAppStore(s => s.loadAweWisdomWell);
  const loadAwePendingDecisions = useAppStore(s => s.loadAwePendingDecisions);
  const submitAweBatchFeedback = useAppStore(s => s.submitAweBatchFeedback);

  useEffect(() => {
    void loadAweSummary();
    void loadAweWisdomWell();
    void loadAwePendingDecisions();
  }, [loadAweSummary, loadAweWisdomWell, loadAwePendingDecisions]);

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
    <div className="divide-y divide-border/30">
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
