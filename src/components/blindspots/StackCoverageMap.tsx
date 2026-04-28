// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useCallback } from 'react';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import { recordTrustEvent } from '../../lib/trust-feedback';
import { ArticleReader } from '../ArticleReader';
import {
  type DepRow, STATUS_CONFIG, URGENCY_COLORS, MAX_SIGNALS_PER_DEP, extractItemId,
} from './types';

const SignalRow = memo(function SignalRow({
  item, onDismiss,
}: {
  item: EvidenceItem;
  onDismiss?: (id: string) => void;
}) {
  const cite = item.evidence[0];
  const numericId = extractItemId(item.id);
  const freshness = cite && cite.freshness_days > 0
    ? `${Math.round(cite.freshness_days)}d ago` : 'today';

  return (
    <div className="px-4 py-2.5 hover:bg-bg-tertiary/30 transition-colors group">
      <div className="flex items-start gap-2.5">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-0.5">
            {cite?.url ? (
              <a
                href={cite.url}
                target="_blank"
                rel="noopener noreferrer"
                className="text-sm text-white hover:text-amber-400 transition-colors leading-snug truncate"
                onClick={() => recordTrustEvent({
                  eventType: 'acted_on', signalId: item.id,
                  sourceType: 'missed_signal', topic: item.title, notes: 'blind_spot_click',
                })}
              >
                {item.title}
              </a>
            ) : (
              <span className="text-sm text-white leading-snug truncate">{item.title}</span>
            )}
            <span className={`text-[10px] px-1.5 py-0.5 rounded shrink-0 ${URGENCY_COLORS[item.urgency]}`}>
              {item.urgency}
            </span>
          </div>
          <div className="flex items-center gap-2 text-xs text-text-muted">
            {cite && (<><span>{cite.source}</span><span>·</span><span>{freshness}</span></>)}
          </div>
          {numericId != null && (
            <div className="mt-1">
              <ArticleReader itemId={numericId} url={cite?.url ?? undefined} contentType={cite?.source} />
            </div>
          )}
        </div>
        {onDismiss && (
          <button
            onClick={() => onDismiss(item.id)}
            className="text-xs text-text-muted hover:text-red-400 opacity-0 group-hover:opacity-100 transition-all shrink-0 px-1.5 py-1 rounded hover:bg-red-500/10"
            title="Not relevant"
          >
            ✕
          </button>
        )}
      </div>
    </div>
  );
});

const DepCoverageRow = memo(function DepCoverageRow({
  dep, onDismissSignal,
}: {
  dep: DepRow;
  onDismissSignal: (id: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const cfg = STATUS_CONFIG[dep.status];
  const hasContent = dep.signals.length > 0 || dep.gap !== null;

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
        <span className="text-sm font-medium text-white flex-1 truncate">{dep.name}</span>
        {dep.projects.length > 0 && (
          <span className="text-[10px] text-text-muted shrink-0 truncate max-w-[120px]" title={dep.projects.join(', ')}>
            {dep.projects.length <= 2
              ? dep.projects.map(p => p.split('/').pop() ?? p).join(', ')
              : `${dep.projects.length} projects`}
          </span>
        )}
        {dep.signals.length > 0 && (
          <span className="text-[10px] text-text-muted shrink-0">
            {dep.signals.length} signal{dep.signals.length === 1 ? '' : 's'}
          </span>
        )}
        <span className={`text-[10px] px-1.5 py-0.5 rounded shrink-0 ${cfg.color}`}>
          {cfg.label}
        </span>
      </button>
      {expanded && hasContent && (
        <div className="bg-bg-tertiary/20 border-t border-border/50">
          {dep.gap && dep.signals.length > 0 && (
            <div className="px-4 py-2.5 text-xs text-text-muted border-b border-border/30">
              {dep.gap.explanation}
            </div>
          )}
          {dep.signals.length > 0 ? (
            <div className="divide-y divide-border/30">
              {dep.signals.slice(0, MAX_SIGNALS_PER_DEP).map(s => (
                <SignalRow key={s.id} item={s} onDismiss={onDismissSignal} />
              ))}
              {dep.signals.length > MAX_SIGNALS_PER_DEP && (
                <div className="px-4 py-2 text-[11px] text-text-muted">
                  +{dep.signals.length - MAX_SIGNALS_PER_DEP} more signal{dep.signals.length - MAX_SIGNALS_PER_DEP === 1 ? '' : 's'}
                </div>
              )}
            </div>
          ) : dep.gap ? (
            <div className="px-4 py-3 text-xs text-text-muted">
              {dep.gap.explanation}
            </div>
          ) : null}
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
  emptyText: string;
}

export const TierSection = memo(function TierSection({
  dotColor, borderColor, title, subtitle,
  badgeText, badgeColor,
  depRows, onDismissSignal, emptyText,
}: TierSectionProps) {
  return (
    <section className="mb-4" aria-label={title}>
      <div className="bg-bg-secondary rounded-lg border overflow-hidden" style={{ borderColor }}>
        <div className="px-4 py-3 border-b border-border flex items-center gap-2">
          <div className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: dotColor }} />
          <h3 className="text-sm font-medium text-white flex-1">{title}</h3>
          <span className="text-xs text-[#8A8A8A]">{subtitle}</span>
          <span className="text-[10px] px-1.5 py-0.5 rounded shrink-0" style={{ color: badgeColor }}>
            {badgeText}
          </span>
        </div>
        {depRows.length > 0 ? (
          <div>
            {depRows.map(dep => (
              <DepCoverageRow key={dep.name} dep={dep} onDismissSignal={onDismissSignal} />
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
  return (
    <section className="mb-4" aria-label="Emerging">
      <div className="bg-bg-secondary rounded-lg border overflow-hidden" style={{ borderColor: 'rgba(59, 130, 246, 0.2)' }}>
        <div className="px-4 py-3 border-b border-border flex items-center gap-2">
          <div className="w-2 h-2 rounded-full shrink-0 bg-blue-500" />
          <h3 className="text-sm font-medium text-white flex-1">Emerging</h3>
          <span className="text-xs text-[#8A8A8A]">
            {items.length} {items.length === 1 ? 'signal' : 'signals'} in your neighborhood
          </span>
          <span className="text-[10px] px-1.5 py-0.5 rounded shrink-0 text-blue-400">Trending</span>
        </div>
        {items.length > 0 ? (
          <div className="divide-y divide-border/50">
            {items.map(it => <SignalRow key={it.id} item={it} onDismiss={onDismiss} />)}
          </div>
        ) : (
          <div className="px-4 py-4">
            <p className="text-xs text-[#8A8A8A]">No emerging signals detected</p>
          </div>
        )}
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
  const [showCovered, setShowCovered] = useState(false);

  if (depRows.length === 0) return null;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <button
        onClick={() => setShowCovered(prev => !prev)}
        className="w-full px-4 py-3 flex items-center gap-2 hover:bg-bg-tertiary/30 transition-colors"
      >
        <div className="w-2 h-2 rounded-full bg-green-400" />
        <h3 className="text-sm font-medium text-white flex-1 text-left">
          Well Covered ({depRows.length})
        </h3>
        <span className="text-[10px] text-green-400">
          {showCovered ? 'Hide' : 'Show'}
        </span>
      </button>
      {showCovered && (
        <div className="border-t border-border">
          {depRows.map(dep => (
            <DepCoverageRow key={dep.name} dep={dep} onDismissSignal={onDismissSignal} />
          ))}
        </div>
      )}
    </div>
  );
});
