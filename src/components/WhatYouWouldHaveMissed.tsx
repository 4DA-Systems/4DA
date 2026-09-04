// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { useShallow } from 'zustand/react/shallow';
import type { SourceRelevance } from '../types/analysis';
import { getRelevancePresentation, isSurfacedSignal } from '../utils/score';
import { useLicense } from '../hooks/use-license';
import { isBriefSuppressed, useActiveBriefFilteredIds } from '../hooks/use-brief-verdicts';
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

/**
 * Priority order for the hero "critical save", security first. Keyed on the
 * canonical SignalKind (not a raw vocab string) so the chooser reads the SAME
 * dual-vocabulary classifier as the label/color — otherwise a real CVE tagged
 * content_type="security_advisory" (signal_type unset) is skipped at the
 * security tier and a lower-priority item wins the hero card.
 */
const KIND_PRIORITY_ORDER: SignalKind[] = ['security', 'breaking', 'tool'];

/** Did the backend confirm this item is genuinely tied to the user's stack? */
function hasConfirmedStackLink(r: SourceRelevance): boolean {
  // Gate on the backend's canonical grounding verdict, NOT dep_match_score or
  // matched_deps length. matched_deps names what the card can display, but the
  // strong-grounding flag is the source of truth for "affects you" placement.
  return r.is_critical_alert === true || r.score_breakdown?.strongly_grounded === true;
}

/**
 * Split a run into what 4DA surfaced and what it rejected, by the ONE
 * definition of signal (`isSurfacedSignal`). The hero pool is drawn from
 * `surfaced` only: an item the pipeline did not call relevant, or that an
 * exclusion demoted, must never be "the one you would have missed".
 */
export function partitionSignal(results: SourceRelevance[]): {
  surfaced: SourceRelevance[];
  rejected: number;
} {
  const surfaced = results.filter(isSurfacedSignal);
  return { surfaced, rejected: results.length - surfaced.length };
}

export function findMostCriticalSave(results: SourceRelevance[]): SourceRelevance | null {
  // A "critical save" is the ONE thing you would have missed — so it must be
  // genuinely tied to the user's stack: a CONCRETE dependency they actually use.
  // Security first. A tool/advisory with no named stack package is a
  // nice-to-know that belongs in Key Signals, never the hero.
  //
  // Deliberately NO fabrication fallback. The prior logic surfaced the top
  // tool_discovery with no dep requirement, then ultimately the highest-scoring
  // item of any kind — which presented a Docker tool with "no confirmed link to
  // your stack" as the daily "critical save". If nothing is stack-grounded we
  // return null and the hero renders an honest "you're clear" state instead of
  // inventing a save. That honesty is the point: the card that sometimes says
  // "you're good" is the one users believe the day it says "you're not".
  for (const kind of KIND_PRIORITY_ORDER) {
    const match = results.find(r => classifySignal(r) === kind && hasConfirmedStackLink(r));
    if (match) return match;
  }
  return null;
}

/**
 * Canonical signal kind used for the critical-save label + color.
 *
 * Only kinds the backend can actually produce. `dependency_update`,
 * `migration_opportunity`, and `architecture_insight` were never wired into the
 * Rust `SignalType` enum (signals.rs) or the `ContentType` vocab, so branches
 * for them could never fire — removed as dead code rather than left as a false
 * promise (the same parallel-vocab drift that caused the security_advisory bug).
 */
type SignalKind = 'security' | 'breaking' | 'tool';

/**
 * Classify the critical save into a canonical signal kind.
 *
 * An item can carry its type in EITHER vocabulary: the signal vocabulary
 * (`signal_type`: security_alert / breaking_change / tool_discovery / ...) or
 * the content vocabulary (`score_breakdown.content_type`: security_advisory /
 * release_notes / show_and_tell / ...). `findMostCriticalSave` already matches
 * on both fields, so the label/color MUST read both too — otherwise a real CVE
 * tagged content_type="security_advisory" (signal_type unset) rendered with no
 * label and the default gold instead of the red "Security advisory" it earned.
 * Checked in the same priority order as findMostCriticalSave (security first).
 */
export function classifySignal(item: SourceRelevance): SignalKind | null {
  const sig = item.signal_type ?? undefined;
  const content = item.score_breakdown?.content_type ?? undefined;
  const has = (v: string) => sig === v || content === v;
  // 'security_advisory' is the content-vocab twin of the 'security_alert' signal.
  if (has('security_alert') || has('security_advisory')) return 'security';
  if (has('breaking_change')) return 'breaking';
  if (has('tool_discovery')) return 'tool';
  return null;
}

export function getSignalLabel(item: SourceRelevance): string | null {
  switch (classifySignal(item)) {
    case 'security': return 'Security advisory';
    case 'breaking': return 'Breaking change';
    case 'tool': return 'Tool discovery';
    default: return null;
  }
}

/**
 * Every signal colour is a design-system token reference, never a hex literal.
 *
 * Both matter. A hex literal is frozen to the dark palette — the light theme
 * redefines `--color-error` (#EF4444 -> #B91C1C) and `--color-accent-action`
 * (#F97316 -> #EA580C), so a hardcoded value silently ignores the theme. And a
 * `var()` reference cannot be turned into a wash by string-concatenating a hex
 * alpha suffix: `var(--color-accent-action)15` is invalid CSS, which the browser
 * drops to `rgba(0, 0, 0, 0)` without warning. Use [`tint`] and [`onTint`].
 */
export function getSignalColor(item: SourceRelevance): string {
  switch (classifySignal(item)) {
    case 'security': return 'var(--color-error)';
    case 'breaking': return 'var(--color-accent-action)';
    default: return 'var(--color-accent-gold)';
  }
}

/** A translucent wash of `color`, valid for a token reference as well as a hex. */
const tint = (color: string, percent: number) =>
  `color-mix(in srgb, ${color} ${percent}%, transparent)`;

/**
 * The same hue pushed toward the theme's own text colour, so small text clears
 * WCAG AA against the washed background it sits on.
 *
 * Measured on the live app: the raw signal colour as 10px text on its own 8%
 * wash reaches only 4.41:1, under the 4.5:1 AA floor for normal text. Mixing
 * 15% of `--color-text-primary` lifts it to 5.22:1. That token is #FFFFFF on
 * dark and #141414 on light, so the mix moves away from the background in both
 * themes rather than lightening unconditionally.
 */
const onTint = (color: string) =>
  `color-mix(in srgb, ${color} 85%, var(--color-text-primary))`;

export const WhatYouWouldHaveMissed = memo(function WhatYouWouldHaveMissed() {
  const { t } = useTranslation();
  const { results, analysisComplete } = useAppStore(
    useShallow(s => ({
      results: s.appState.relevanceResults,
      analysisComplete: s.appState.analysisComplete,
    })),
  );

  const { isPro } = useLicense();

  // AD-035: the hero pick honors the latest briefing's filter verdicts —
  // an item the briefing called noise must not be today's "critical save".
  // Stats (scanned/rejected counts) stay truthful over the FULL result set;
  // only the hero selection is bound. is_critical_alert items are exempt.
  const briefFilteredIds = useActiveBriefFilteredIds();

  const insight = useMemo(() => {
    if (!analysisComplete || results.length === 0) return null;

    // Same predicate as the header's "N relevant" chip — see isSurfacedSignal.
    // A `top_score >= 0.35` filter lived here until 2026-09-04 and put a
    // second, larger "signal" number on the same screen as the header's.
    const { surfaced: relevant, rejected } = partitionSignal(results);
    const totalScanned = results.length;
    const rejectionRate = totalScanned > 0 ? ((rejected / totalScanned) * 100).toFixed(1) : '0';
    const heroPool = relevant.filter(r => !isBriefSuppressed(r, briefFilteredIds));
    if (heroPool.length < relevant.length) {
      console.info(
        `[brief-verdicts] ${relevant.length - heroPool.length} hero candidate(s) demoted by the latest briefing's verdicts`,
      );
    }
    const criticalSave = findMostCriticalSave(heroPool);

    return { relevant, totalScanned, rejected, rejectionRate, criticalSave };
  }, [results, analysisComplete, briefFilteredIds]);

  if (!insight || insight.totalScanned < 5) return null;

  const { relevant, totalScanned, rejected, rejectionRate, criticalSave } = insight;
  const signalLabel = criticalSave ? getSignalLabel(criticalSave) : null;
  const signalColor = criticalSave ? getSignalColor(criticalSave) : 'var(--color-accent-gold)';

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
          <SignalUpgradeCTA compact source="missed-teaser" />
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
              <div className="text-2xl font-bold font-mono text-text-primary">{rejected}</div>
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
          </div>

          {/* Rejection rate badge */}
          <div className="ms-auto px-2.5 py-1 rounded-full bg-accent-gold/10 border border-accent-gold/20">
            <span className="text-xs font-mono font-medium text-accent-gold">{rejectionRate}%</span>
            <span className="text-[10px] text-text-muted ms-1">
              {t('missed.filtered')}
            </span>
          </div>
        </div>

        {/* The critical save — "this is the one" — or an honest "you're clear"
            state when nothing is genuinely tied to the user's stack. */}
        {criticalSave ? (
          <div
            className="rounded-lg p-3 border"
            style={{
              backgroundColor: tint(signalColor, 3),
              borderColor: tint(signalColor, 13),
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
                      color: onTint(signalColor),
                      backgroundColor: tint(signalColor, 8),
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
                    className="text-sm text-text-primary font-medium truncate hover:text-accent-gold transition-colors text-left cursor-pointer"
                  >
                    {criticalSave.title}
                  </button>
                ) : (
                  <p className="text-sm text-text-primary font-medium truncate">
                    {criticalSave.title}
                  </p>
                )}
                <p className="text-xs text-text-muted mt-1">
                  {criticalSave.explanation || criticalSave.source_type}
                  {/* eslint-disable i18next/no-literal-string */}
                  {criticalSave.score_breakdown?.matched_deps?.length ? (
                    <span className="text-text-secondary">
                      {' '}&middot; matches: {criticalSave.score_breakdown.matched_deps.slice(0, 3).join(', ')}
                    </span>
                  ) : null}
                  {/* eslint-enable i18next/no-literal-string */}
                </p>
              </div>
              <div className="text-end flex-shrink-0">
                <div
                  className={`text-sm font-medium uppercase tracking-wider ${getRelevancePresentation(criticalSave.top_score).colorClass}`}
                >
                  {t(getRelevancePresentation(criticalSave.top_score).labelKey)}
                </div>
              </div>
            </div>
          </div>
        ) : (
          <div
            className="rounded-lg p-3 border"
            style={{ backgroundColor: '#22C55E0F', borderColor: '#22C55E33' }}
          >
            <div className="flex items-start gap-3">
              <div
                className="w-1 self-stretch min-h-[36px] rounded-full flex-shrink-0"
                style={{ backgroundColor: '#22C55E' }}
              />
              <div className="flex-1 min-w-0">
                <p className="text-sm text-text-primary font-medium">{t('missed.clearTitle')}</p>
                <p className="text-xs text-text-muted mt-1">
                  {t('missed.clearBody', { relevant: relevant.length })}
                </p>
              </div>
            </div>
          </div>
        )}

        {/* Grounded summary — only claims the numbers the pipeline actually
            computed (items scanned, items filtered). The old counterfactual
            ("would have been buried in N other items") asserted an alternate
            reality the system cannot verify. */}
        <p className="text-[11px] text-text-muted text-center">
          {t('missed.grounded', {
            total: totalScanned,
            rejected,
          })}
        </p>
      </div>
    </div>
  );
});

