// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { useShallow } from 'zustand/react/shallow';
import type { SourceRelevance } from '../types/analysis';
import { useLicense } from '../hooks/use-license';
import { SignalUpgradeCTA } from './SignalUpgradeCTA';

/**
 * "What You Would Have Missed" — the most persuasive feature in 4DA.
 *
 * Takes today's analysis results and tells the user: out of N items scanned,
 * 4DA surfaced K that matter. Here's the ONE you would have missed — the
 * security advisory for a package in YOUR Cargo.toml, the breaking change
 * in YOUR dependency, the opportunity that matched YOUR exact stack.
 *
 * This is the feature that makes users think "I can never go back."
 */

const SIGNAL_PRIORITY_ORDER = [
  'security_alert',
  'breaking_change',
  'dependency_update',
  'migration_opportunity',
  'tool_discovery',
  'architecture_insight',
];

function findMostCriticalSave(results: SourceRelevance[]): SourceRelevance | null {
  // For security items, require dependency confirmation — an irrelevant CVE as hero card destroys trust
  for (const priority of SIGNAL_PRIORITY_ORDER) {
    const isSecurityType = priority === 'security_alert' || priority === 'breaking_change';
    const match = results.find(
      r => (r.score_breakdown?.content_type === priority || r.signal_type === priority)
        && (!isSecurityType || (r.score_breakdown?.dep_match_score ?? 0) > 0.2)
    );
    if (match) return match;
  }

  // Fallback: security items without dep match (still better than nothing)
  for (const priority of SIGNAL_PRIORITY_ORDER) {
    const match = results.find(
      r => r.score_breakdown?.content_type === priority || r.signal_type === priority
    );
    if (match) return match;
  }

  // Fallback: highest dependency match score
  const withDeps = results.filter(r => (r.score_breakdown?.dep_match_score ?? 0) > 0.2);
  if (withDeps.length > 0) {
    return withDeps.sort((a, b) =>
      (b.score_breakdown?.dep_match_score ?? 0) - (a.score_breakdown?.dep_match_score ?? 0)
    )[0] ?? null;
  }

  // Final fallback: highest scoring item
  return results.length > 0
    ? results.reduce((best, r) => r.top_score > best.top_score ? r : best)
    : null;
}

function formatTimeSaved(totalScanned: number): string {
  // Average 8 seconds per article to scan/evaluate manually
  const minutes = Math.round((totalScanned * 8) / 60);
  if (minutes < 60) return `${minutes} min`;
  const hours = (minutes / 60).toFixed(1);
  return `${hours} hr`;
}

function getSignalLabel(item: SourceRelevance): string | null {
  const type = item.score_breakdown?.content_type || item.signal_type;
  switch (type) {
    case 'security_alert': return 'Security advisory';
    case 'breaking_change': return 'Breaking change';
    case 'dependency_update': return 'Dependency update';
    case 'migration_opportunity': return 'Migration opportunity';
    case 'tool_discovery': return 'Tool discovery';
    case 'architecture_insight': return 'Architecture insight';
    default: return null;
  }
}

function getSignalColor(item: SourceRelevance): string {
  const type = item.score_breakdown?.content_type || item.signal_type;
  switch (type) {
    case 'security_alert': return '#EF4444';
    case 'breaking_change': return 'var(--color-accent-action)';
    case 'dependency_update': return '#3B82F6';
    default: return '#D4AF37';
  }
}

export const WhatYouWouldHaveMissed = memo(function WhatYouWouldHaveMissed() {
  const { t } = useTranslation();
  const { results, analysisComplete } = useAppStore(
    useShallow(s => ({
      results: s.appState.relevanceResults,
      analysisComplete: s.appState.analysisComplete,
    })),
  );

  const { isPro } = useLicense();

  const insight = useMemo(() => {
    if (!analysisComplete || results.length === 0) return null;

    const relevant = results.filter(r => r.top_score >= 0.35);
    const totalScanned = results.length;
    const rejected = totalScanned - relevant.length;
    const rejectionRate = totalScanned > 0 ? ((rejected / totalScanned) * 100).toFixed(1) : '0';
    const criticalSave = findMostCriticalSave(relevant);

    return { relevant, totalScanned, rejected, rejectionRate, criticalSave };
  }, [results, analysisComplete]);

  if (!insight || insight.totalScanned < 5) return null;

  const { relevant, totalScanned, rejected, rejectionRate, criticalSave } = insight;
  const timeSaved = formatTimeSaved(rejected);
  const signalLabel = criticalSave ? getSignalLabel(criticalSave) : null;
  const signalColor = criticalSave ? getSignalColor(criticalSave) : '#D4AF37';

  // Only show when there's a compelling story (enough rejection + a critical save)
  if (relevant.length === 0 || parseFloat(rejectionRate) < 80) return null;

  // Free tier: compelling teaser without full analytics
  if (!isPro) {
    return (
      <div className="mb-5 bg-bg-secondary border border-border rounded-xl overflow-hidden">
        <div className="px-4 py-3 border-b border-border/50 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-accent-gold" />
            <span className="text-xs font-medium text-accent-gold">
              {t('missed.title')}
            </span>
          </div>
          <span className="text-[10px] text-text-muted">
            {t('missed.scanned', { count: totalScanned })}
          </span>
        </div>
        <div className="px-4 py-5 space-y-3">
          <p className="text-sm text-text-secondary text-center">
            {t('missed.freeTeaser', {
              rejected,
              relevant: relevant.length,
            })}
          </p>
          <p className="text-xs text-text-muted text-center">
            {t('missed.freeSubtext')}
          </p>
          <SignalUpgradeCTA compact />
        </div>
      </div>
    );
  }

  return (
    <div className="mb-5 bg-bg-secondary border border-border rounded-xl overflow-hidden">
      {/* Header bar */}
      <div className="px-4 py-3 border-b border-border/50 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 rounded-full bg-accent-gold" />
          <span className="text-xs font-medium text-accent-gold">
            {t('missed.title')}
          </span>
        </div>
        <span className="text-[10px] text-text-muted">
          {t('missed.scanned', { count: totalScanned })}
        </span>
      </div>

      <div className="p-4 space-y-3">
        {/* The stats */}
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-6">
            <div>
              <div className="text-2xl font-bold font-mono text-white">{rejected}</div>
              <div className="text-[10px] text-text-muted">
                {t('missed.noiseRejected')}
              </div>
            </div>
            <div className="w-px h-8 bg-border/50" />
            <div>
              <div className="text-2xl font-bold font-mono text-success">{relevant.length}</div>
              <div className="text-[10px] text-text-muted">
                {t('missed.signalSurfaced')}
              </div>
            </div>
            <div className="w-px h-8 bg-border/50" />
            <div>
              <div className="text-2xl font-bold font-mono text-text-secondary">{timeSaved}</div>
              <div className="text-[10px] text-text-muted">
                {t('missed.timeSaved')}
              </div>
            </div>
          </div>

          {/* Rejection rate badge */}
          <div className="ms-auto px-2.5 py-1 rounded-full bg-accent-gold/10 border border-accent-gold/20">
            <span className="text-xs font-mono font-medium text-accent-gold">{rejectionRate}%</span>
            <span className="text-[10px] text-text-muted ms-1">
              {t('missed.filtered')}
            </span>
          </div>
        </div>

        {/* The critical save — "this is the one" */}
        {criticalSave && (
          <div
            className="rounded-lg p-3 border"
            style={{
              backgroundColor: `${signalColor}08`,
              borderColor: `${signalColor}20`,
            }}
          >
            <div className="flex items-start gap-3">
              <div
                className="w-1 h-full min-h-[40px] rounded-full flex-shrink-0"
                style={{ backgroundColor: signalColor }}
              />
              <div className="flex-1 min-w-0">
                {signalLabel && (
                  <span
                    className="inline-block text-[10px] font-medium px-1.5 py-0.5 rounded mb-1.5"
                    style={{
                      color: signalColor,
                      backgroundColor: `${signalColor}15`,
                    }}
                  >
                    {signalLabel}
                  </span>
                )}
                {criticalSave.url ? (
                  <button
                    onClick={() => {
                      import('@tauri-apps/plugin-opener').then(({ openUrl }) => {
                        void openUrl(criticalSave.url!);
                      }).catch(() => {
                        window.open(criticalSave.url!, '_blank', 'noopener,noreferrer');
                      });
                    }}
                    className="text-sm text-white font-medium truncate hover:text-accent-gold transition-colors text-left cursor-pointer"
                  >
                    {criticalSave.title}
                  </button>
                ) : (
                  <p className="text-sm text-white font-medium truncate">
                    {criticalSave.title}
                  </p>
                )}
                <p className="text-xs text-text-muted mt-1">
                  {criticalSave.explanation || criticalSave.source_type}
                  {criticalSave.score_breakdown?.matched_deps?.length ? (
                    <span className="text-text-secondary">
                      {' '}&middot; matches: {criticalSave.score_breakdown.matched_deps.slice(0, 3).join(', ')}
                    </span>
                  ) : null}
                </p>
              </div>
              <div className="text-end flex-shrink-0">
                <div
                  className="text-lg font-bold font-mono"
                  style={{ color: signalColor }}
                >
                  {Math.round(criticalSave.top_score * 100)}
                </div>
                <div className="text-[9px] text-text-muted">score</div>
              </div>
            </div>
          </div>
        )}

        {/* The persuasion line */}
        <p className="text-[11px] text-text-muted text-center">
          {t('missed.persuasion', {
            count: rejected,
          })}
        </p>
      </div>
    </div>
  );
});

export default WhatYouWouldHaveMissed;
