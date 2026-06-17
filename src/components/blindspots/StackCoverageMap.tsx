// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useCallback, useEffect } from 'react';
import { isSafeUrl } from '../../utils/sanitize-html';
import { useTranslation } from 'react-i18next';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import { recordTrustEvent } from '../../lib/trust-feedback';
import { useTranslatedContent } from '../ContentTranslationProvider';
import { ArticleReader } from '../ArticleReader';
import {
  type DepRow, STATUS_CONFIG, URGENCY_COLORS, MAX_SIGNALS_PER_DEP, extractItemId,
  sourceTypeLabel,
} from './types';

// Strip the " (ecosystem)" qualifier from a display name → bare package name.
export function bareName(displayName: string): string {
  return displayName.replace(/\s*\([^)]*\)\s*$/, '').trim();
}

// Best-effort ecosystem for a dep row: from the "(eco)" suffix, falling back to
// the bs_uncov_<eco>_ id prefix.
export function depEcosystem(dep: DepRow): string {
  const m = dep.name.match(/\(([^)]+)\)\s*$/);
  if (m) return m[1]!.trim();
  if (dep.gap?.id.startsWith('bs_uncov_')) {
    return dep.gap.id.replace('bs_uncov_', '').split('_')[0] ?? '';
  }
  return '';
}

// Build the canonical registry/search URL for "Investigate" on a dependency.
export function investigateUrl(name: string, ecosystem: string): string {
  const eco = ecosystem.toLowerCase();
  const pkg = encodeURIComponent(name);
  if (eco === 'crates.io' || eco === 'cargo' || eco === 'rust') return `https://crates.io/crates/${pkg}`;
  if (eco === 'npm' || eco === 'javascript' || eco === 'typescript') return `https://www.npmjs.com/package/${pkg}`;
  if (eco === 'pypi' || eco === 'python') return `https://pypi.org/project/${pkg}`;
  if (eco === 'go' || eco === 'golang') return `https://pkg.go.dev/${pkg}`;
  return `https://www.google.com/search?q=${encodeURIComponent(`${name} ${ecosystem}`)}`;
}

const lastInvestigateRef = { current: 0 };
function openInvestigate(dep: DepRow): void {
  const now = Date.now();
  if (now - lastInvestigateRef.current < 500) return; // debounce double-clicks
  lastInvestigateRef.current = now;
  const url = investigateUrl(bareName(dep.name), depEcosystem(dep));
  recordTrustEvent({ eventType: 'acted_on', sourceType: 'gap', topic: dep.name, notes: 'blind_spot_investigate' });
  import('@tauri-apps/plugin-opener')
    .then(({ openUrl }) => openUrl(url))
    .catch(() => window.open(url, '_blank', 'noopener,noreferrer'));
}

const SignalRow = memo(function SignalRow({
  item, onDismiss,
}: {
  item: EvidenceItem;
  onDismiss?: (id: string) => void;
}) {
  const { t } = useTranslation();
  const { getTranslated, requestTranslation } = useTranslatedContent();
  const cite = item.evidence[0];
  const numericId = extractItemId(item.id);
  useEffect(() => {
    const reqs = [{ id: item.id, text: item.title }];
    if (cite?.relevance_note) reqs.push({ id: `${item.id}:rel`, text: cite.relevance_note });
    requestTranslation(reqs);
  }, [item.id, item.title, cite, requestTranslation]);
  const displayTitle = getTranslated(item.id, item.title);
  const freshness = (() => {
    if (!cite || cite.freshness_days <= 0) return t('preemption.freshness.today');
    const d = Math.round(cite.freshness_days);
    if (d === 1) return t('preemption.freshness.yesterday');
    if (d < 7) return t('preemption.freshness.daysAgo', { count: d });
    if (d < 30) return t('preemption.freshness.weeksAgo', { count: Math.floor(d / 7) });
    return t('preemption.freshness.monthsAgo', { count: Math.floor(d / 30) });
  })();

  return (
    <div className="px-4 py-2.5 hover:bg-bg-tertiary/30 transition-colors group">
      <div className="flex items-start gap-2.5">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-0.5">
            {cite?.url && isSafeUrl(cite.url) ? (
              <a
                href={cite.url}
                target="_blank"
                rel="noopener noreferrer"
                className="text-sm text-text-primary hover:text-amber-400 transition-colors leading-snug truncate"
                onClick={() => recordTrustEvent({
                  eventType: 'acted_on', signalId: item.id,
                  sourceType: 'missed_signal', topic: item.title, notes: 'blind_spot_click',
                })}
              >
                {displayTitle}
              </a>
            ) : (
              <span className="text-sm text-text-primary leading-snug truncate">{displayTitle}</span>
            )}
            <span className={`text-[10px] px-1.5 py-0.5 rounded shrink-0 ${URGENCY_COLORS[item.urgency]}`}>
              {t(`preemption.urgency.${item.urgency}`)}
            </span>
          </div>
          <div className="flex items-center gap-2 text-xs text-text-muted">
            {cite && (() => {
              const stl = sourceTypeLabel(cite.source);
              return (
                <>
                  {stl && <span className={`text-[10px] ${stl.color}`}>{stl.label}</span>}
                  <span>{cite.source}</span><span>·</span><span>{freshness}</span>
                </>
              );
            })()}
            {cite?.relevance_note && (
              <span className="text-[10px] text-amber-400/70 px-1.5 py-0.5 bg-amber-400/5 rounded">
                {getTranslated(`${item.id}:rel`, cite.relevance_note)}
              </span>
            )}
          </div>
          {numericId != null && (
            <div className="mt-1">
              <ArticleReader itemId={numericId} url={cite?.url ?? undefined} contentType={cite?.source} />
            </div>
          )}
        </div>
        {/* eslint-disable i18next/no-literal-string */}
        {onDismiss && (
          <button
            onClick={() => onDismiss(item.id)}
            className="text-xs text-text-muted hover:text-red-400 opacity-0 group-hover:opacity-100 transition-all shrink-0 px-1.5 py-1 rounded hover:bg-red-500/10"
            title={t('blindspots.signal.notRelevant')}
          >
            ✕
          </button>
        )}
        {/* eslint-enable i18next/no-literal-string */}
      </div>
    </div>
  );
});

const DepCoverageRow = memo(function DepCoverageRow({
  dep, onDismissSignal, onAddWatch,
}: {
  dep: DepRow;
  onDismissSignal: (id: string) => void;
  onAddWatch?: (packageName: string, ecosystem: string) => void;
}) {
  const { t } = useTranslation();
  const { getTranslated, requestTranslation } = useTranslatedContent();
  const [expanded, setExpanded] = useState(false);
  const [showAllSignals, setShowAllSignals] = useState(false);
  const cfg = STATUS_CONFIG[dep.status];
  const hasContent = dep.signals.length > 0 || dep.gap !== null;
  useEffect(() => {
    if (!dep.gap) return;
    const reqs = [{ id: `${dep.gap.id}:expl`, text: dep.gap.explanation }];
    const rel = dep.gap.evidence[0]?.relevance_note;
    if (rel) reqs.push({ id: `${dep.gap.id}:rel`, text: rel });
    requestTranslation(reqs);
  }, [dep.gap, requestTranslation]);
  const gapExplanation = dep.gap ? getTranslated(`${dep.gap.id}:expl`, dep.gap.explanation) : '';

  const handleToggle = useCallback(() => {
    if (!hasContent) return;
    setExpanded(prev => !prev);
    if (!expanded) {
      recordTrustEvent({
        eventType: 'acted_on', sourceType: 'gap',
        topic: dep.name, notes: 'stack_map_expand',
      });
    }
  }, [expanded, dep.name, hasContent]);

  return (
    <div className="border-b border-border/50 last:border-b-0">
      <button
        onClick={handleToggle}
        disabled={!hasContent}
        className={`w-full px-4 py-3 flex items-center gap-3 text-left transition-colors ${
          hasContent ? 'hover:bg-bg-tertiary/30 cursor-pointer' : 'cursor-default'
        }`}
      >
        {hasContent && (
          <span className={`text-[10px] transition-transform duration-150 text-text-muted ${expanded ? 'rotate-90' : ''}`}>▶</span>
        )}
        {!hasContent && <span className="w-[10px]" />}
        <div className={`w-2 h-2 rounded-full shrink-0 ${cfg.dot}`} />
        <span className="text-sm font-medium text-text-primary shrink-0">{dep.name}</span>
        {dep.gap && !expanded && (
          <span className="text-[11px] text-text-muted truncate flex-1">
            — {gapExplanation}
          </span>
        )}
        {!dep.gap && <span className="flex-1" />}
        {dep.projects.length > 0 && (
          <span className="text-[10px] text-text-muted shrink-0 truncate max-w-[120px]" title={dep.projects.join(', ')}>
            {dep.projects.length <= 2
              ? dep.projects.map(p => p.split('/').pop() ?? p).join(', ')
              : t('blindspots.projects.count', { count: dep.projects.length })}
          </span>
        )}
        {dep.signals.length > 0 && (
          <span className="text-[10px] text-text-muted shrink-0">
            {t('blindspots.signal.count', { count: dep.signals.length })}
          </span>
        )}
        {dep.gap?.lens_hints.other_build_target && (
          <span
            className="text-[9px] px-1.5 py-0.5 rounded shrink-0 text-text-muted bg-bg-tertiary border border-border uppercase tracking-wider"
            title={t('blindspots.otherTargets.badgeHint')}
          >
            {t('blindspots.otherTargets.badge')}
          </span>
        )}
        <span className={`text-[10px] px-1.5 py-0.5 rounded shrink-0 ${cfg.color}`}>
          {t(cfg.labelKey)}
        </span>
      </button>
      {expanded && hasContent && (
        <div className="bg-bg-tertiary/20 border-t border-border/50">
          {dep.gap && (
            <div className={`px-4 py-2.5 ${dep.signals.length > 0 ? 'border-b border-border/30' : ''}`}>
              <p className="text-xs text-text-muted">{gapExplanation}</p>
              {dep.gap.evidence[0]?.relevance_note && (
                <p className="text-[10px] text-text-muted/70 mt-1">{getTranslated(`${dep.gap.id}:rel`, dep.gap.evidence[0].relevance_note)}</p>
              )}
              {/* Phase A: actions grouped + always visible, where the eye lands —
                  no more hover-hidden + buried on the far right. */}
              <div className="mt-2.5 flex flex-wrap items-center gap-2">
                {onAddWatch && (
                  <button
                    onClick={() => onAddWatch(bareName(dep.name), depEcosystem(dep))}
                    className="inline-flex items-center text-[11px] px-2 py-1 rounded-md border border-border bg-bg-tertiary/60 text-text-secondary hover:text-green-400 hover:border-green-500/30 hover:bg-green-500/10 transition-colors"
                    title={t('blindspots.action.watch')}
                  >
                    {t('blindspots.action.watchLabel')}
                  </button>
                )}
                <button
                  onClick={() => openInvestigate(dep)}
                  className="inline-flex items-center text-[11px] px-2 py-1 rounded-md border border-border bg-bg-tertiary/60 text-text-secondary hover:text-amber-400 hover:border-amber-500/30 hover:bg-amber-500/10 transition-colors"
                  title={t('blindspots.action.investigate')}
                >
                  {t('blindspots.action.investigate')}
                </button>
                <button
                  onClick={() => onDismissSignal(dep.gap!.id)}
                  className="inline-flex items-center text-[11px] px-2 py-1 rounded-md border border-border bg-bg-tertiary/60 text-text-secondary hover:text-red-400 hover:border-red-500/30 hover:bg-red-500/10 transition-colors"
                  title={t('blindspots.signal.notRelevant')}
                >
                  {t('blindspots.action.dismiss')}
                </button>
              </div>
            </div>
          )}
          {dep.signals.length > 0 && (
            <div className="divide-y divide-border/30">
              {(showAllSignals ? dep.signals : dep.signals.slice(0, MAX_SIGNALS_PER_DEP)).map(s => (
                <SignalRow key={s.id} item={s} onDismiss={onDismissSignal} />
              ))}
              {dep.signals.length > MAX_SIGNALS_PER_DEP && (
                <button
                  onClick={(e) => { e.stopPropagation(); setShowAllSignals(prev => !prev); }}
                  className="w-full px-4 py-2 text-[11px] text-text-muted hover:text-amber-400 transition-colors text-left"
                >
                  {showAllSignals
                    ? t('blindspots.signal.showLess')
                    : t('blindspots.signal.more', { count: dep.signals.length - MAX_SIGNALS_PER_DEP })}
                </button>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
});

interface TierSectionProps {
  dotColor: string;
  borderColor: string;
  title: string;
  subtitle: string;
  badgeText: string;
  badgeColor: string;
  depRows: DepRow[];
  onDismissSignal: (id: string) => void;
  onAddWatch?: (packageName: string, ecosystem: string) => void;
  emptyText: string;
}

export const TierSection = memo(function TierSection({
  dotColor, borderColor, title, subtitle,
  badgeText, badgeColor,
  depRows, onDismissSignal, onAddWatch, emptyText,
}: TierSectionProps) {
  return (
    <section className="mb-4" aria-label={title}>
      <div className="bg-bg-secondary rounded-lg border overflow-hidden" style={{ borderColor }}>
        <div className="px-4 py-3 border-b border-border flex items-center gap-2">
          <div className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: dotColor }} />
          <h3 className="text-sm font-medium text-text-primary flex-1">{title}</h3>
          <span className="text-xs text-[#8A8A8A]">{subtitle}</span>
          <span className="text-[10px] px-1.5 py-0.5 rounded shrink-0" style={{ color: badgeColor }}>
            {badgeText}
          </span>
        </div>
        {depRows.length > 0 ? (
          <div>
            {depRows.map(dep => (
              <DepCoverageRow key={dep.name} dep={dep} onDismissSignal={onDismissSignal} onAddWatch={onAddWatch} />
            ))}
          </div>
        ) : (
          <div className="px-4 py-4">
            <p className="text-xs text-[#8A8A8A]">{emptyText}</p>
          </div>
        )}
      </div>
    </section>
  );
});

export const EmergingSignals = memo(function EmergingSignals({
  items, onDismiss,
}: {
  items: EvidenceItem[];
  onDismiss: (id: string) => void;
}) {
  const { t } = useTranslation();

  if (items.length === 0) return null;

  return (
    <section className="mb-4" aria-label={t('blindspots.emerging.title')}>
      <div className="bg-bg-secondary rounded-lg border overflow-hidden" style={{ borderColor: 'rgba(59, 130, 246, 0.2)' }}>
        <div className="px-4 py-3 border-b border-border flex items-center gap-2">
          <div className="w-2 h-2 rounded-full shrink-0 bg-blue-500" />
          <h3 className="text-sm font-medium text-text-primary flex-1">{t('blindspots.emerging.title')}</h3>
          <span className="text-xs text-[#8A8A8A]">
            {t('blindspots.emerging.subtitle', { count: items.length })}
          </span>
          <span className="text-[10px] px-1.5 py-0.5 rounded shrink-0 text-blue-400">{t('blindspots.emerging.trending')}</span>
        </div>
        <div className="divide-y divide-border/50">
          {items.map(it => <SignalRow key={it.id} item={it} onDismiss={onDismiss} />)}
        </div>
      </div>
    </section>
  );
});

export const CoveredSection = memo(function CoveredSection({
  depRows, onDismissSignal,
}: {
  depRows: DepRow[];
  onDismissSignal: (id: string) => void;
}) {
  const { t } = useTranslation();
  const [showCovered, setShowCovered] = useState(false);
  const [detailView, setDetailView] = useState(false);

  if (depRows.length === 0) return null;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <button
        onClick={() => setShowCovered(prev => !prev)}
        className="w-full px-4 py-3 flex items-center gap-2 hover:bg-bg-tertiary/30 transition-colors"
      >
        <div className="w-2 h-2 rounded-full bg-green-400" />
        <h3 className="text-sm font-medium text-text-primary flex-1 text-left">
          {t('blindspots.covered.title')} ({depRows.length})
        </h3>
        <span className="text-[10px] text-green-400">
          {showCovered ? t('blindspots.covered.hide') : t('blindspots.covered.show')}
        </span>
      </button>
      {showCovered && (
        <div className="border-t border-border">
          {!detailView ? (
            <div className="px-4 py-3">
              <div className="flex flex-wrap gap-1.5">
                {depRows.map(dep => (
                  <span
                    key={dep.name}
                    className="text-[11px] px-2 py-1 rounded bg-green-500/8 text-green-400/80 border border-green-500/10"
                  >
                    {dep.name}
                  </span>
                ))}
              </div>
              {depRows.some(dep => dep.signals.length > 0) && (
                <button
                  onClick={(e) => { e.stopPropagation(); setDetailView(true); }}
                  className="mt-2.5 text-[10px] text-text-muted hover:text-green-400 transition-colors"
                >
                  {t('blindspots.covered.details')}
                </button>
              )}
            </div>
          ) : (
            <div>
              <button
                onClick={(e) => { e.stopPropagation(); setDetailView(false); }}
                className="w-full px-4 py-1.5 text-[10px] text-text-muted hover:text-green-400 transition-colors text-right"
              >
                {t('blindspots.covered.compact')}
              </button>
              {depRows.map(dep => (
                <DepCoverageRow key={dep.name} dep={dep} onDismissSignal={onDismissSignal} />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
});

/**
 * Phase 2c: a collapsed group for dependencies whose coverage gap applies only
 * to a build target the user does NOT build on the host (e.g. a `cfg(not(windows))`
 * crate on Windows). Surfaced, de-prioritised, never hidden — a cross-platform
 * dev can expand it. Modeled on `CoveredSection`.
 */
export const OtherBuildTargetsSection = memo(function OtherBuildTargetsSection({
  depRows, onDismissSignal, onAddWatch,
}: {
  depRows: DepRow[];
  onDismissSignal: (id: string) => void;
  onAddWatch?: (packageName: string, ecosystem: string) => void;
}) {
  const { t } = useTranslation();
  const [show, setShow] = useState(false);

  if (depRows.length === 0) return null;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <button
        onClick={() => setShow(prev => !prev)}
        aria-expanded={show}
        className="w-full px-4 py-3 flex items-center gap-2 hover:bg-bg-tertiary/30 transition-colors"
      >
        <div className="w-2 h-2 rounded-full bg-[#8A8A8A]" />
        <h3 className="text-sm font-medium text-text-secondary flex-1 text-left">
          {t('blindspots.otherTargets.show', { count: depRows.length })}
        </h3>
        <span className="text-[10px] text-text-muted">
          {show ? t('blindspots.otherTargets.hide') : t('blindspots.otherTargets.expand')}
        </span>
      </button>
      {show && (
        <div className="border-t border-border">
          {depRows.map(dep => (
            <DepCoverageRow key={dep.name} dep={dep} onDismissSignal={onDismissSignal} onAddWatch={onAddWatch} />
          ))}
        </div>
      )}
    </div>
  );
});
