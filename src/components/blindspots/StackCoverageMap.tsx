// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useCallback } from 'react';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import { recordTrustEvent } from '../../lib/trust-feedback';
import { ArticleReader } from '../ArticleReader';
import {
  type DepRow, STATUS_CONFIG, URGENCY_COLORS, MAX_SIGNALS_PER_DEP, extractItemId,
} from './types';

// ============================================================================
// Signal Row
// ============================================================================

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

// ============================================================================
// Dep Coverage Row
// ============================================================================

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
          <span className="text-[10px] text-text-muted shrink-0">
            {dep.projects.length} project{dep.projects.length === 1 ? '' : 's'}
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
          {dep.gap && (
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
          ) : (
            <div className="px-4 py-3 text-xs text-text-muted italic">
              No recent signals found — potential gap in source coverage.
            </div>
          )}
        </div>
      )}
    </div>
  );
});

// ============================================================================
// Stack Coverage Map
// ============================================================================

export const StackCoverageMap = memo(function StackCoverageMap({
  depRows, onDismissSignal,
}: {
  depRows: DepRow[];
  onDismissSignal: (id: string) => void;
}) {
  const blindSpots = depRows.filter(d => d.status === 'blind_spot');
  const fallingBehind = depRows.filter(d => d.status === 'falling_behind');
  const covered = depRows.filter(d => d.status === 'well_covered');
  const [showCovered, setShowCovered] = useState(false);

  return (
    <div className="space-y-3">
      {blindSpots.length > 0 && (
        <div className="bg-bg-secondary rounded-lg border border-red-500/20 overflow-hidden">
          <div className="px-4 py-3 border-b border-border flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-red-400" />
            <h3 className="text-sm font-medium text-white flex-1">
              Blind Spots ({blindSpots.length})
            </h3>
            <span className="text-[10px] text-red-400">Needs attention</span>
          </div>
          <div>
            {blindSpots.map(dep => (
              <DepCoverageRow key={dep.name} dep={dep} onDismissSignal={onDismissSignal} />
            ))}
          </div>
        </div>
      )}

      {fallingBehind.length > 0 && (
        <div className="bg-bg-secondary rounded-lg border border-yellow-500/15 overflow-hidden">
          <div className="px-4 py-3 border-b border-border flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-yellow-400" />
            <h3 className="text-sm font-medium text-white flex-1">
              Falling Behind ({fallingBehind.length})
            </h3>
            <span className="text-[10px] text-yellow-400">Drifting</span>
          </div>
          <div>
            {fallingBehind.map(dep => (
              <DepCoverageRow key={dep.name} dep={dep} onDismissSignal={onDismissSignal} />
            ))}
          </div>
        </div>
      )}

      {covered.length > 0 && (
        <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
          <button
            onClick={() => setShowCovered(prev => !prev)}
            className="w-full px-4 py-3 flex items-center gap-2 hover:bg-bg-tertiary/30 transition-colors"
          >
            <div className="w-2 h-2 rounded-full bg-green-400" />
            <h3 className="text-sm font-medium text-white flex-1 text-left">
              Well Covered ({covered.length})
            </h3>
            <span className="text-[10px] text-green-400">
              {showCovered ? 'Hide' : 'Show'}
            </span>
          </button>
          {showCovered && (
            <div className="border-t border-border">
              {covered.map(dep => (
                <DepCoverageRow key={dep.name} dep={dep} onDismissSignal={onDismissSignal} />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
});

// ============================================================================
// Unmatched Signals
// ============================================================================

export const UnmatchedSignals = memo(function UnmatchedSignals({
  items, onDismiss,
}: {
  items: EvidenceItem[];
  onDismiss: (id: string) => void;
}) {
  if (items.length === 0) return null;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-4 py-3 border-b border-border">
        <h3 className="text-sm font-medium text-white">
          Other Signals ({items.length})
        </h3>
        <p className="text-[10px] text-text-muted mt-0.5">Relevant but not tied to a specific dependency</p>
      </div>
      <div className="divide-y divide-border/50">
        {items.map(it => <SignalRow key={it.id} item={it} onDismiss={onDismiss} />)}
      </div>
    </div>
  );
});
