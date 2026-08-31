// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import type { Urgency } from '../../../src-tauri/bindings/bindings/Urgency';
import { cmd } from '../../lib/commands';
import { recordTrustEvent } from '../../lib/trust-feedback';
import { useTranslatedContent } from '../ContentTranslationProvider';

export const URGENCY_CONFIG: Record<
  Urgency,
  { color: string; bg: string; border: string; dot: string; labelKey: string }
> = {
  critical: {
    color: 'text-red-400',
    bg: 'bg-red-500/[0.06]',
    border: 'border-red-500/25',
    dot: 'bg-red-400',
    labelKey: 'preemption.urgency.critical',
  },
  high: {
    color: 'text-orange-400',
    bg: 'bg-orange-500/[0.05]',
    border: 'border-orange-500/25',
    dot: 'bg-orange-400',
    labelKey: 'preemption.urgency.high',
  },
  medium: {
    color: 'text-yellow-400',
    bg: 'bg-yellow-500/[0.04]',
    border: 'border-yellow-500/20',
    dot: 'bg-yellow-400',
    labelKey: 'preemption.urgency.medium',
  },
  watch: {
    color: 'text-blue-400',
    bg: 'bg-blue-500/[0.04]',
    border: 'border-blue-500/20',
    dot: 'bg-blue-400',
    labelKey: 'preemption.urgency.watch',
  },
};

export const URGENCY_ORDER: Urgency[] = ['critical', 'high', 'medium', 'watch'];

const EVIDENCE_COLLAPSE_THRESHOLD = 2;
const EXPLANATION_MAX_LENGTH = 280;

function getTierStyle(provenance: string): {
  badge: string | null;
  badgeClass: string;
  borderClass: string;
} {
  if (provenance === 'osv_verified') {
    return {
      badge: 'preemption.badge.verified',
      badgeClass: 'text-emerald-400 bg-emerald-500/10 border border-emerald-500/20',
      borderClass: 'border-l-2 border-l-emerald-500/60',
    };
  }
  if (provenance === 'llm_assessed') {
    return {
      badge: 'preemption.badge.ai',
      badgeClass: 'text-blue-400 bg-blue-500/10 border border-blue-500/20',
      borderClass: 'border-l-2 border-l-blue-500/40',
    };
  }
  return { badge: null, badgeClass: '', borderClass: '' };
}

function formatFreshness(days: number, t: (key: string, opts?: Record<string, unknown>) => string): string {
  const d = Math.round(days);
  if (d <= 0) return t('preemption.freshness.today');
  if (d === 1) return t('preemption.freshness.yesterday');
  if (d < 7) return t('preemption.freshness.daysAgo', { count: d });
  if (d < 30) return t('preemption.freshness.weeksAgo', { count: Math.floor(d / 7) });
  return t('preemption.freshness.monthsAgo', { count: Math.floor(d / 30) });
}

function truncateAt(text: string, limit: number): string {
  if (text.length <= limit) return text;
  const cut = text.slice(0, limit);
  const lastSpace = cut.lastIndexOf(' ');
  return `${lastSpace > limit - 40 ? cut.slice(0, lastSpace) : cut}…`;
}

function kindAsSourceType(item: EvidenceItem): string {
  return typeof item.kind === 'string' ? item.kind : String(item.kind);
}

function shortenProjectPath(fullPath: string): string {
  const parts = fullPath.replace(/\\/g, '/').split('/').filter(Boolean);
  if (parts.length <= 2) return parts.join('/');
  return parts.slice(-2).join('/');
}

function formatProjectNames(paths: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const p of paths) {
    const short = shortenProjectPath(p);
    if (!seen.has(short)) {
      seen.add(short);
      out.push(short);
    }
  }
  return out;
}

const EvidenceList = memo(function EvidenceList({
  evidence,
  cardTitle,
  hiddenExtra,
  onExpand,
}: {
  evidence: EvidenceItem['evidence'];
  cardTitle?: string;
  /**
   * Citations the LIST transport held back server-side (AD-035) — counted in
   * the header and the "show more" control; `onExpand` fetches them.
   */
  hiddenExtra: number;
  onExpand?: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  const filtered = cardTitle
    ? evidence.filter(e => e.title.toLowerCase() !== cardTitle.toLowerCase())
    : evidence;

  if (filtered.length === 0 && hiddenExtra === 0) return null;

  const shown = expanded ? filtered : filtered.slice(0, EVIDENCE_COLLAPSE_THRESHOLD);
  const canCollapse = filtered.length > EVIDENCE_COLLAPSE_THRESHOLD || hiddenExtra > 0;

  return (
    <div className="mt-3 pt-3 border-t border-border/50">
      <h4 className="text-[10px] font-medium text-text-muted uppercase tracking-wider mb-2">
        {t('preemption.evidence')} ({evidence.length + hiddenExtra})
      </h4>
      <ul className="space-y-1.5">
        {shown.map((e, i) => (
          <li key={i} className="flex items-baseline gap-2 text-xs min-w-0">
            <span className="shrink-0 font-mono text-[10px] uppercase text-text-muted w-14 truncate">
              {e.source}
            </span>
            {e.url ? (
              <a
                href={e.url}
                target="_blank"
                rel="noopener noreferrer"
                className="flex-1 min-w-0 text-text-secondary hover:text-text-primary transition-colors truncate"
                title={e.title}
              >
                {e.title}
              </a>
            ) : (
              <span
                className="flex-1 min-w-0 text-text-secondary truncate"
                title={e.title}
              >
                {e.title}
              </span>
            )}
            <span className="shrink-0 text-[10px] text-text-muted tabular-nums">
              {formatFreshness(e.freshness_days, t)}
            </span>
          </li>
        ))}
      </ul>
      {canCollapse && (
        <button
          type="button"
          onClick={() => {
            if (expanded) {
              setExpanded(false);
              return;
            }
            // Lazy detail fetch (AD-035): the list shipped only the rendered
            // rows; the rest hydrate from get_preemption_item_detail. On a
            // fetch failure the embedded rows still expand.
            void (async () => {
              try { await onExpand?.(); } catch { /* keep embedded rows */ }
              setExpanded(true);
            })();
          }}
          className="mt-2 text-[11px] text-text-muted hover:text-text-secondary transition-colors"
        >
          {expanded
            ? t('preemption.evidence.showLess')
            : t('preemption.evidence.showMore', {
                count: Math.max(filtered.length - EVIDENCE_COLLAPSE_THRESHOLD, 0) + hiddenExtra,
              })}
        </button>
      )}
    </div>
  );
});

const AffectedChips = memo(function AffectedChips({
  item,
}: {
  item: EvidenceItem;
}) {
  const { t } = useTranslation();
  const projectNames = formatProjectNames(item.affected_projects);
  const hasProjects = projectNames.length > 0;
  const hasDeps = item.affected_deps.length > 0;
  if (!hasProjects && !hasDeps) return null;

  return (
    <div className="mt-3 space-y-1.5 text-xs">
      {hasProjects && (
        <div className="flex items-baseline gap-2 flex-wrap">
          <span className="shrink-0 text-[10px] font-medium text-text-muted uppercase tracking-wider w-16">
            {t('preemption.affected.projects')}
          </span>
          <div className="flex flex-wrap gap-1">
            {projectNames.slice(0, 4).map((name) => (
              <span
                key={name}
                className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-bg-tertiary text-text-secondary border border-border"
              >
                {name}
              </span>
            ))}
            {projectNames.length > 4 && (
              <span className="text-[10px] text-text-muted">+{projectNames.length - 4}</span>
            )}
          </div>
        </div>
      )}
      {hasDeps && (
        <div className="flex items-baseline gap-2 flex-wrap">
          <span className="shrink-0 text-[10px] font-medium text-text-muted uppercase tracking-wider w-16">
            {t('preemption.affected.deps')}
          </span>
          <div className="flex flex-wrap gap-1">
            {item.affected_deps.slice(0, 6).map((dep) => (
              <span
                key={dep}
                className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-bg-tertiary text-text-secondary border border-border"
              >
                {dep}
              </span>
            ))}
            {item.affected_deps.length > 6 && (
              <span className="inline-flex items-center px-1.5 py-0.5 text-[10px] text-text-muted">
                +{item.affected_deps.length - 6}
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
});

const lastClickRef = { current: 0 };

export const ItemCard = memo(function ItemCard({
  item,
  surfacedRef,
  onDismiss,
}: {
  item: EvidenceItem;
  surfacedRef: React.RefObject<Set<string>>;
  onDismiss: (id: string) => void;
}) {
  const { t } = useTranslation();
  const { getTranslated, requestTranslation } = useTranslatedContent();
  const [explanationExpanded, setExplanationExpanded] = useState(false);
  // Lazy hydration (AD-035): the LIST response embeds only what the collapsed
  // card renders (evidence capped with `evidence_total` recording the real
  // count, explanation byte-capped, tooltips blanked). The first expansion
  // fetches the complete item once and renders from it thereafter.
  const [fullItem, setFullItem] = useState<EvidenceItem | null>(null);
  const hydratingRef = useRef<Promise<void> | null>(null);
  const shown = fullItem ?? item;
  const isListTrimmed = item.evidence_total != null;
  const evidenceHeldBack = fullItem
    ? 0
    : Math.max((item.evidence_total ?? item.evidence.length) - item.evidence.length, 0);

  const hydrate = useCallback((): Promise<void> => {
    if (fullItem || !isListTrimmed) return Promise.resolve();
    hydratingRef.current ??= cmd('get_preemption_item_detail', { itemId: item.id })
      .then(detail => { setFullItem(detail); })
      .finally(() => { hydratingRef.current = null; });
    return hydratingRef.current;
  }, [fullItem, isListTrimmed, item.id]);

  const cfg = URGENCY_CONFIG[item.urgency] ?? URGENCY_CONFIG.watch;
  const tier = getTierStyle(item.confidence.provenance);
  const sourceType = kindAsSourceType(item);

  useEffect(() => {
    if (!surfacedRef.current.has(item.id)) {
      surfacedRef.current.add(item.id);
      recordTrustEvent({
        eventType: 'surfaced',
        alertId: item.id,
        sourceType,
        topic: item.title,
      });
    }
  }, [item.id, sourceType, item.title, surfacedRef]);

  useEffect(() => {
    const reqs = [{ id: item.id, text: item.title }];
    if (item.explanation) reqs.push({ id: `${item.id}:expl`, text: item.explanation });
    const vc = item.evidence.find(e => e.source === 'version_context');
    if (vc) reqs.push({ id: `${item.id}:vc`, text: vc.title });
    requestTranslation(reqs);
  }, [item.id, item.title, item.explanation, item.evidence, requestTranslation]);

  // The hydrated explanation is the FULL text — translate it under its own
  // key so a cached translation of the transport-capped text can't shadow it.
  useEffect(() => {
    if (fullItem?.explanation) {
      requestTranslation([{ id: `${fullItem.id}:expl:full`, text: fullItem.explanation }]);
    }
  }, [fullItem, requestTranslation]);

  const displayTitle = getTranslated(item.id, item.title);
  const explanationText = fullItem
    ? getTranslated(`${fullItem.id}:expl:full`, fullItem.explanation)
    : getTranslated(`${item.id}:expl`, item.explanation);
  // A transport-capped explanation always re-expands past the display clamp,
  // so length alone still decides whether the "more" control renders.
  const needsTruncation = explanationText.length > EXPLANATION_MAX_LENGTH;
  const displayedExplanation = needsTruncation && !explanationExpanded
    ? truncateAt(explanationText, EXPLANATION_MAX_LENGTH)
    : explanationText;

  return (
    <article className={`rounded-lg border ${cfg.border} ${cfg.bg} overflow-hidden ${tier.borderClass}`}>
      <header className="px-4 pt-4 pb-3">
        <div className="flex items-start gap-3">
          <span
            className={`shrink-0 inline-flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wider px-2 py-1 rounded ${cfg.color} bg-black/20 border ${cfg.border}`}
          >
            <span className={`w-1.5 h-1.5 rounded-full ${cfg.dot}`} />
            {t(cfg.labelKey)}
          </span>
          {tier.badge && (
            <span className={`shrink-0 inline-flex items-center text-[9px] font-semibold uppercase tracking-wider px-1.5 py-0.5 rounded ${tier.badgeClass}`}>
              {t(tier.badge)}
            </span>
          )}
          {item.lens_hints.other_build_target && (
            <span
              className="shrink-0 inline-flex items-center text-[9px] font-semibold uppercase tracking-wider px-1.5 py-0.5 rounded text-text-muted bg-bg-tertiary border border-border"
              title={t('preemption.otherTargets.badgeHint')}
            >
              {t('preemption.otherTargets.badge')}
            </span>
          )}
          <h3 className="flex-1 min-w-0 text-[13px] font-medium text-text-primary leading-snug">
            {displayTitle}
          </h3>
          <span
            className="shrink-0 text-[10px] font-mono tabular-nums text-text-muted"
            title={t('preemption.confidence.provenance', {
              provenance: item.confidence.provenance,
              sampleSize: item.confidence.sample_size ? ` (n=${item.confidence.sample_size})` : '',
            })}
          >
            {Math.round(item.confidence.value * 100)}%
          </span>
        </div>
      </header>
      <div className="px-4 pb-4">
        {(() => {
          const versionCite = item.evidence.find(e => e.source === 'version_context');
          if (!versionCite) return null;
          return (
            <div className="flex items-center gap-2 mt-1 mb-2 px-2.5 py-1.5 rounded bg-black/20 border border-border text-[11px] font-mono text-text-secondary">
              {getTranslated(`${item.id}:vc`, versionCite.title)}
            </div>
          );
        })()}
        {item.explanation && (
          <p className="text-xs text-text-secondary leading-relaxed">
            {displayedExplanation}
            {needsTruncation && (
              <button
                type="button"
                onClick={() => {
                  if (explanationExpanded) {
                    setExplanationExpanded(false);
                    return;
                  }
                  // Hydrate first so a transport-capped explanation expands
                  // to the FULL text, not to the capped tail (AD-035).
                  void hydrate()
                    .catch(() => { /* keep the shipped text */ })
                    .finally(() => { setExplanationExpanded(true); });
                }}
                className="ms-1 text-text-muted hover:text-text-secondary underline-offset-2 hover:underline"
              >
                {explanationExpanded
                  ? t('preemption.explanation.collapse', 'less')
                  : t('preemption.explanation.expand', 'more')}
              </button>
            )}
          </p>
        )}
        <AffectedChips item={item} />
        <EvidenceList
          evidence={shown.evidence.filter(e => e.source !== 'version_context')}
          cardTitle={item.title}
          hiddenExtra={evidenceHeldBack}
          onExpand={hydrate}
        />
        {shown.suggested_actions.length > 0 && (
          <div className="mt-4 flex flex-wrap gap-2">
            {shown.suggested_actions.map((action, i) => (
              <button
                key={i}
                type="button"
                className="px-3 py-1.5 text-[11px] rounded-md border border-border bg-bg-tertiary/60 text-text-secondary hover:text-text-primary hover:bg-bg-tertiary hover:border-text-primary/20 transition-colors"
                title={action.description}
                onClick={() => {
                  recordTrustEvent({
                    eventType: action.action_id === 'dismiss' ? 'dismissed' : 'acted_on',
                    alertId: item.id,
                    sourceType,
                    topic: item.title,
                    notes: action.label,
                  });
                  if (action.action_id === 'dismiss' || action.action_id === 'snooze_7d') {
                    onDismiss(item.id);
                  } else if (action.action_id === 'investigate' || action.action_id === 'view_source') {
                    const now = Date.now();
                    if (now - lastClickRef.current < 500) return;
                    lastClickRef.current = now;
                    const url = shown.evidence[0]?.url
                      ?? `https://www.google.com/search?q=${encodeURIComponent(item.title)}`;
                    import('@tauri-apps/plugin-opener')
                      .then(({ openUrl }) => openUrl(url))
                      .catch(() => window.open(url, '_blank', 'noopener,noreferrer'));
                  }
                }}
              >
                {action.label}
              </button>
            ))}
          </div>
        )}
      </div>
    </article>
  );
});
