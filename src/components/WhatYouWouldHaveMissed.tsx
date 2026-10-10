// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { useShallow } from 'zustand/react/shallow';
import type { SourceRelevance } from '../types/analysis';
import { getRelevancePresentation, isSurfacedSignal } from '../utils/score';
import { isBriefSuppressed, useActiveBriefFilteredIds } from '../hooks/use-brief-verdicts';
import { isGrounded } from './signals/evidence-pool';
import { orderByStackTier, stackTier } from './signals/signal-lanes';

/**
 * "What You Would Have Missed" — the ONE surfaced item genuinely tied to the
 * user's stack (the security advisory for a package in THEIR Cargo.toml, the
 * breaking change in THEIR dependency), or an honest "you're clear".
 *
 * The card carries no counters (doctrine rule 3, audit 2026-10-02). It used to
 * lead with "3011 noise rejected / 314 signal surfaced / 90.6% filtered /
 * ranked from 3325 items scanned": the denominator was everything fetched and
 * merged, and the card only rendered at a >= 80% rejection rate, so the number
 * could only ever look impressive — it informed no action. The hero item is
 * the information; the counts were decoration.
 */

/**
 * Priority order for the hero "critical save", security first. Keyed on the
 * canonical SignalKind (not a raw vocab string) so the chooser reads the SAME
 * dual-vocabulary classifier as the label/color — otherwise a real CVE tagged
 * content_type="security_advisory" (signal_type unset) is skipped at the
 * security tier and a lower-priority item wins the hero card.
 */
const KIND_PRIORITY_ORDER: SignalKind[] = ['security', 'breaking', 'tool'];

/**
 * The hero candidates: the surfaced set only, by the ONE definition of signal
 * (`isSurfacedSignal`, same as the header chip). An item the pipeline did not
 * call relevant, or that an exclusion demoted, must never be "the one you
 * would have missed".
 */
export function heroCandidates(results: SourceRelevance[]): SourceRelevance[] {
  return results.filter(isSurfacedSignal);
}

/**
 * The hero is drawn from grounded feed rows (`isGrounded`, the Affects-You
 * pool) ordered by the feed's stack tiering (`orderByStackTier`: security,
 * then breaking / deprecation). It
 * used to keep its own rules (`signal_type` / `content_type` only), so a
 * graded "Breaking upgrade: typescript 6.0.3 -> 7.0.2" -- a breaking change by
 * `necessity_category`, `release_notes` by content type -- was invisible to it:
 * the card said "No security, breaking change, or dependency alert" directly
 * above that very row in the lane (fresh-profile E2E 2026-10-09).
 */
export function findMostCriticalSave(results: SourceRelevance[]): SourceRelevance | null {
  // A "critical save" is the ONE thing you would have missed, so it must be
  // genuinely tied to the user's stack. Security first. Deliberately NO
  // fabrication fallback: if nothing grounded is security / breaking / a tool
  // release, the card renders an honest state instead of inventing a save.
  const stackOrdered = orderByStackTier(results.filter(isGrounded));
  for (const kind of KIND_PRIORITY_ORDER) {
    const match = stackOrdered.find(r => heroKind(r) === kind);
    if (match) return match;
  }
  return null;
}

/** Kind as the "Your stack" lane tiers it, falling back to the raw vocabularies. */
function heroKind(r: SourceRelevance): SignalKind | null {
  const tier = stackTier(r);
  if (tier === 0) return 'security';
  if (tier === 1) return 'breaking';
  return classifySignal(r);
}

/** Does any grounded item carry a security or breaking-change event? */
export function hasStackAlert(results: SourceRelevance[]): boolean {
  return results.some(r => isGrounded(r) && stackTier(r) <= 1);
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
  switch (heroKind(item)) {
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
  switch (heroKind(item)) {
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

const CLEAR_COLOR = 'var(--color-success)';

export const WhatYouWouldHaveMissed = memo(function WhatYouWouldHaveMissed() {
  const { t } = useTranslation();
  const { results, analysisComplete } = useAppStore(
    useShallow(s => ({
      results: s.appState.relevanceResults,
      analysisComplete: s.appState.analysisComplete,
    })),
  );

  // AD-035: the hero pick honors the latest briefing's filter verdicts —
  // an item the briefing called noise must not be today's "critical save".
  // is_critical_alert items are exempt.
  const briefFilteredIds = useActiveBriefFilteredIds();

  const insight = useMemo(() => {
    if (!analysisComplete || results.length === 0) return null;
    const relevant = heroCandidates(results);
    const heroPool = relevant.filter(r => !isBriefSuppressed(r, briefFilteredIds));
    if (heroPool.length < relevant.length) {
      console.info(
        `[brief-verdicts] ${relevant.length - heroPool.length} hero candidate(s) demoted by the latest briefing's verdicts`,
      );
    }
    // The non-hero copy reads the WHOLE surfaced set (brief verdicts
    // included): the lane below lists those rows, so the card may only claim
    // "no security / breaking change" when the lane truly has none.
    return {
      relevantCount: relevant.length,
      criticalSave: findMostCriticalSave(heroPool),
      stackAlert: hasStackAlert(relevant),
      stackUpdates: relevant.some(isGrounded),
    };
  }, [results, analysisComplete, briefFilteredIds]);

  // Nothing surfaced → nothing to say; the feed's own empty state speaks.
  if (!insight || insight.relevantCount === 0) return null;

  const { relevantCount, criticalSave, stackAlert, stackUpdates } = insight;
  const clearTitle = stackAlert ? t('missed.stackReviewTitle') : t('missed.clearTitle');
  const clearBody = stackAlert
    ? t('missed.stackReviewBody')
    : stackUpdates
      ? t('missed.clearBodyUpdates')
      : t('missed.clearBody', { relevant: relevantCount });
  const signalLabel = criticalSave ? getSignalLabel(criticalSave) : null;
  const signalColor = criticalSave ? getSignalColor(criticalSave) : 'var(--color-accent-gold)';

  return (
    <div className="mb-5 bg-bg-secondary border border-border rounded-xl overflow-hidden">
      <div className="px-4 py-3 border-b border-border/50 flex items-center gap-2">
        <div className="w-2 h-2 rounded-full bg-accent-gold" />
        <span className="text-xs font-medium text-accent-gold">
          {t('missed.title')}
        </span>
      </div>

      <div className="p-4">
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
            style={{ backgroundColor: tint(CLEAR_COLOR, 6), borderColor: tint(CLEAR_COLOR, 20) }}
          >
            <div className="flex items-start gap-3">
              <div
                className="w-1 self-stretch min-h-[36px] rounded-full flex-shrink-0"
                style={{ backgroundColor: CLEAR_COLOR }}
              />
              <div className="flex-1 min-w-0">
                <p className="text-sm text-text-primary font-medium">{clearTitle}</p>
                <p className="text-xs text-text-muted mt-1">{clearBody}</p>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
});
