// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import type { Urgency } from '../../../src-tauri/bindings/bindings/Urgency';
import { recordTrustEvent } from '../../lib/trust-feedback';

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
}: {
  evidence: EvidenceItem['evidence'];
  cardTitle?: string;
}) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  const filtered = cardTitle
    ? evidence.filter(e => e.title.toLowerCase() !== cardTitle.toLowerCase())
    : evidence;

  if (filtered.length === 0) return null;

  const shown = expanded ? filtered : filtered.slice(0, EVIDENCE_COLLAPSE_THRESHOLD);
  const canCollapse = filtered.length > EVIDENCE_COLLAPSE_THRESHOLD;

  return (
    <div className="mt-3 pt-3 border-t border-border/50">
      <h4 className="text-[10px] font-medium text-text-muted uppercase tracking-wider mb-2">
        {t('preemption.evidence')} ({evidence.length})
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
                className="flex-1 min-w-0 text-text-secondary hover:text-white transition-colors truncate"
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
          onClick={() => { setExpanded(v => !v); }}
          className="mt-2 text-[11px] text-text-muted hover:text-text-secondary transition-colors"
        >
          {expanded
            ? t('preemption.evidence.showLess')
            : t('preemption.evidence.showMore', { count: filtered.length - EVIDENCE_COLLAPSE_THRESHOLD })}
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
  const [explanationExpanded, setExplanationExpanded] = useState(false);
  const cfg = URGENCY_CONFIG[item.urgency] ?? URGENCY_CONFIG.watch;
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

  const needsTruncation = item.explanation.length > EXPLANATION_MAX_LENGTH;
  const displayedExplanation = needsTruncation && !explanationExpanded
    ? truncateAt(item.explanation, EXPLANATION_MAX_LENGTH)
    : item.explanation;

  return (
    <article className={`rounded-lg border ${cfg.border} ${cfg.bg} overflow-hidden`}>
      <header className="px-4 pt-4 pb-3">
        <div className="flex items-start gap-3">
          <span
            className={`shrink-0 inline-flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wider px-2 py-1 rounded ${cfg.color} bg-black/20 border ${cfg.border}`}
          >
            <span className={`w-1.5 h-1.5 rounded-full ${cfg.dot}`} />
            {t(cfg.labelKey)}
          </span>
          <h3 className="flex-1 min-w-0 text-[13px] font-medium text-white leading-snug">
            {item.title}
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
        {item.explanation && (
          <p className="text-xs text-text-secondary leading-relaxed">
            {displayedExplanation}
            {needsTruncation && (
              <button
                type="button"
                onClick={() => { setExplanationExpanded(v => !v); }}
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
        <EvidenceList evidence={item.evidence} cardTitle={item.title} />
        {item.suggested_actions.length > 0 && (
          <div className="mt-4 flex flex-wrap gap-2">
            {item.suggested_actions.map((action, i) => (
              <button
                key={i}
                type="button"
                className="px-3 py-1.5 text-[11px] rounded-md border border-border bg-bg-tertiary/60 text-text-secondary hover:text-white hover:bg-bg-tertiary hover:border-white/20 transition-colors"
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
                    const url = item.evidence[0]?.url
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
