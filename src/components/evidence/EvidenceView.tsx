// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Evidence Tab — proof that 4DA is working and learning you.
 *
 * Three sections, each backed by verified working Tauri commands:
 *   1. This Week — preemption + blind spot counts
 *   2. Intelligence Pulse — noise rejection, accuracy, learning narratives
 *   3. What 4DA Learned — topic affinities + anti-topics from real feedback
 *
 * Zero AWE dependency. Every data source verified on FREE tier.
 */

import { memo, useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import type { EvidenceFeed } from '../../../src-tauri/bindings/bindings/EvidenceFeed';

// ============================================================================
// Section 1 — This Week
// ============================================================================

const ThisWeekSection = memo(function ThisWeekSection({
  preemptionCount,
  blindSpotCount,
}: {
  preemptionCount: number;
  blindSpotCount: number;
}) {
  const { t } = useTranslation();
  const total = preemptionCount + blindSpotCount;

  return (
    <section className="bg-bg-secondary rounded-lg border border-border p-5">
      <h2 className="text-[10px] text-text-muted uppercase tracking-wider mb-3">
        {t('evidence.thisWeek')}
      </h2>
      {total === 0 ? (
        <p className="text-sm text-text-secondary">{t('evidence.thisWeekEmpty')}</p>
      ) : (
        <div className="flex items-baseline gap-6">
          <div>
            <span className="text-2xl font-semibold text-white tabular-nums">{preemptionCount}</span>
            <span className="text-xs text-text-muted ml-1.5">{t('evidence.preemptiveAlerts')}</span>
          </div>
          <div>
            <span className="text-2xl font-semibold text-white tabular-nums">{blindSpotCount}</span>
            <span className="text-xs text-text-muted ml-1.5">{t('evidence.blindSpotItems')}</span>
          </div>
        </div>
      )}
    </section>
  );
});

// ============================================================================
// Section 2 — Intelligence Pulse
// ============================================================================

interface PulseData {
  items_analyzed_7d: number;
  items_surfaced_7d: number;
  rejection_rate: number;
  calibration_accuracy: number;
  total_cycles: number;
  learning_narratives: string[];
}

const PulseSection = memo(function PulseSection({ pulse }: { pulse: PulseData | null }) {
  const { t } = useTranslation();
  if (!pulse) return null;

  const rejectionPct = Math.round(pulse.rejection_rate);
  const accuracyPct = Math.round(pulse.calibration_accuracy);

  return (
    <section className="bg-bg-secondary rounded-lg border border-border p-5">
      <h2 className="text-[10px] text-text-muted uppercase tracking-wider mb-4">
        {t('evidence.pulse')}
      </h2>

      {/* Stats row */}
      <div className="grid grid-cols-3 gap-4 mb-4">
        <div>
          <div className="text-lg font-semibold text-white tabular-nums">
            {pulse.items_analyzed_7d.toLocaleString()}
          </div>
          <div className="text-[10px] text-text-muted">{t('evidence.pulseAnalyzed')}</div>
        </div>
        <div>
          <div className="text-lg font-semibold text-white tabular-nums">
            {pulse.items_surfaced_7d}
          </div>
          <div className="text-[10px] text-text-muted">{t('evidence.pulseSurfaced')}</div>
        </div>
        <div>
          <div className={`text-lg font-semibold tabular-nums ${rejectionPct >= 70 ? 'text-green-400' : 'text-white'}`}>
            {rejectionPct}%
          </div>
          <div className="text-[10px] text-text-muted">{t('evidence.pulseNoise')}</div>
          {pulse.items_analyzed_7d > 0 && (
            <div className="text-[9px] text-text-muted tabular-nums mt-0.5">
              {pulse.items_analyzed_7d.toLocaleString()} → {pulse.items_surfaced_7d}
            </div>
          )}
        </div>
      </div>

      {/* Calibration + cycles */}
      {(accuracyPct > 0 || pulse.total_cycles > 0) && (
        <div className="flex items-center gap-4 text-xs text-text-secondary border-t border-border/50 pt-3 mb-3">
          {accuracyPct > 0 && (
            <span>{t('evidence.pulseCalibration')} <span className="text-white tabular-nums">{accuracyPct}%</span></span>
          )}
          {pulse.total_cycles > 0 && (
            <span>{t('evidence.pulseCycles')} <span className="text-white tabular-nums">{pulse.total_cycles}</span></span>
          )}
        </div>
      )}

      {/* Learning narratives */}
      {pulse.learning_narratives.length > 0 && (
        <div className="space-y-1.5">
          {pulse.learning_narratives.slice(0, 3).map((narrative, i) => (
            <p key={i} className="text-xs text-text-secondary leading-relaxed flex items-start gap-2">
              <span className="text-accent-gold/50 mt-0.5 shrink-0" aria-hidden="true">&#x25C6;</span>
              <span>{narrative}</span>
            </p>
          ))}
        </div>
      )}
    </section>
  );
});

// ============================================================================
// Section 3 — What 4DA Learned
// ============================================================================

interface TopicAffinity {
  topic: string;
  positive_signals: number;
  negative_signals: number;
  affinity_score: number;
}

interface AntiTopic {
  topic: string;
  rejection_count: number;
}

const LearnedSection = memo(function LearnedSection({
  affinities,
  antiTopics,
}: {
  affinities: TopicAffinity[];
  antiTopics: AntiTopic[];
}) {
  const { t } = useTranslation();
  const hasData = affinities.length > 0 || antiTopics.length > 0;

  return (
    <section className="bg-bg-secondary rounded-lg border border-border p-5">
      <h2 className="text-[10px] text-text-muted uppercase tracking-wider mb-4">
        {t('evidence.learned')}
      </h2>

      {!hasData ? (
        <p className="text-xs text-text-muted leading-relaxed">
          {t('evidence.learnedEmpty')}
        </p>
      ) : (
        <div className="space-y-4">
          {/* Positive affinities */}
          {affinities.length > 0 && (
            <div>
              <h3 className="text-[10px] text-green-400/80 uppercase tracking-wider mb-2">
                {t('evidence.learnedPositive')}
              </h3>
              <div className="space-y-1.5">
                {affinities.slice(0, 6).map(a => {
                  const total = a.positive_signals + a.negative_signals;
                  const savePct = total > 0 ? Math.round((a.positive_signals / total) * 100) : 0;
                  return (
                    <div key={a.topic} className="flex items-center gap-3">
                      <span className="text-xs text-white w-24 truncate font-mono">{a.topic}</span>
                      <div className="flex-1 h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
                        <div
                          className="h-full bg-green-400/60 rounded-full"
                          style={{ width: `${Math.min(100, Math.abs(a.affinity_score) * 100)}%` }}
                        />
                      </div>
                      <span className="text-[10px] text-text-muted tabular-nums w-10 text-right">
                        {savePct}%
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* Anti-topics */}
          {antiTopics.length > 0 && (
            <div>
              <h3 className="text-[10px] text-red-400/80 uppercase tracking-wider mb-2">
                {t('evidence.learnedNegative')}
              </h3>
              <div className="flex flex-wrap gap-1.5">
                {antiTopics.slice(0, 8).map(at => (
                  <span
                    key={at.topic}
                    className="px-2 py-0.5 rounded text-[10px] font-mono bg-red-500/10 text-red-400/80 border border-red-500/20"
                  >
                    {at.topic} ({at.rejection_count})
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </section>
  );
});

// ============================================================================
// Main View
// ============================================================================

const EvidenceView = memo(function EvidenceView() {
  const { t } = useTranslation();
  const [preemptionCount, setPreemptionCount] = useState(0);
  const [blindSpotCount, setBlindSpotCount] = useState(0);
  const [pulse, setPulse] = useState<PulseData | null>(null);
  const [affinities, setAffinities] = useState<TopicAffinity[]>([]);
  const [antiTopics, setAntiTopics] = useState<AntiTopic[]>([]);
  const [loading, setLoading] = useState(true);

  const loadAll = useCallback(async () => {
    setLoading(true);

    const results = await Promise.allSettled([
      cmd('get_preemption_alerts'),
      cmd('get_blind_spots'),
      cmd('get_intelligence_pulse'),
      cmd('ace_get_topic_affinities'),
      cmd('ace_get_anti_topics', { min_rejections: 3 }),
    ]);

    if (results[0].status === 'fulfilled') {
      setPreemptionCount((results[0].value as unknown as EvidenceFeed).total);
    }
    if (results[1].status === 'fulfilled') {
      setBlindSpotCount((results[1].value as unknown as EvidenceFeed).total);
    }
    if (results[2].status === 'fulfilled') {
      setPulse(results[2].value as PulseData);
    }
    if (results[3].status === 'fulfilled') {
      const raw = results[3].value as { affinities: TopicAffinity[] };
      setAffinities(raw.affinities ?? []);
    }
    if (results[4].status === 'fulfilled') {
      const raw = results[4].value as { anti_topics: AntiTopic[] };
      setAntiTopics(raw.anti_topics ?? []);
    }

    setLoading(false);
  }, []);

  useEffect(() => { void loadAll(); }, [loadAll]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20 text-text-secondary text-sm">
        {t('evidence.loading')}
      </div>
    );
  }

  return (
    <div className="space-y-4 pb-8" role="tabpanel" id="view-panel-evidence">
      <header className="mb-2">
        <h1 className="text-xl font-semibold text-white tracking-tight">
          {t('evidence.title')}
        </h1>
        <p className="text-sm text-text-muted mt-1">
          {t('evidence.subtitle')}
        </p>
      </header>

      <ThisWeekSection
        preemptionCount={preemptionCount}
        blindSpotCount={blindSpotCount}
      />

      <PulseSection pulse={pulse} />

      <LearnedSection
        affinities={affinities}
        antiTopics={antiTopics}
      />
    </div>
  );
});

export default EvidenceView;
