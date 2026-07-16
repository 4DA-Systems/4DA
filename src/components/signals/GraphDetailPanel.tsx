// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useState, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd, type GraphNodeDetail } from '../../lib/commands';
import { useAppStore } from '../../store';
import { recordTrustEvent } from '../../lib/trust-feedback';
import { isSafeUrl } from '../../utils/sanitize-html';
import { getSourceLabel } from '../../config/sources';
import { getRelevancePresentation } from '../../utils/score';
import type { SourceRelevance } from '../../types';
import { CATEGORY_COLORS, AFFECTS_GOLD, type ContentNode } from './ContentGraphNode';

interface GraphDetailPanelProps {
  nodeId: number;
  data: ContentNode['data'];
  onClose: () => void;
}

function formatTimeAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  if (Number.isNaN(diff) || diff < 0) return '';
  const mins = Math.floor(diff / 60_000);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}

/** Minimal item shape the learning loop actually reads (title/source/score). */
function toFeedbackItem(title: string, sourceType: string, score: number): SourceRelevance {
  return { id: 0, title, source_type: sourceType, top_score: score } as unknown as SourceRelevance;
}

function openExternal(url: string) {
  import('@tauri-apps/plugin-opener')
    .then(({ openUrl }) => openUrl(url))
    .catch(() => window.open(url, '_blank', 'noopener,noreferrer'));
}

export default function GraphDetailPanel({ nodeId, data, onClose }: GraphDetailPanelProps) {
  const { t } = useTranslation();
  const recordInteraction = useAppStore((s) => s.recordInteraction);
  const feedback = useAppStore((s) => s.feedbackGiven[nodeId]);

  const [details, setDetails] = useState<GraphNodeDetail[] | null>(null);
  const [detailsError, setDetailsError] = useState(false);
  const [summary, setSummary] = useState<string | null>(null);
  const [summaryLoading, setSummaryLoading] = useState(false);
  const [summaryError, setSummaryError] = useState<string | null>(null);

  const memberIds = useMemo(
    () => (data.member_ids?.length ? data.member_ids : [nodeId]),
    [data.member_ids, nodeId],
  );

  const loadDetails = useCallback(() => {
    setDetailsError(false);
    cmd('get_graph_node_details', { itemIds: memberIds })
      .then((rows) => {
        setDetails(rows);
        const rep = rows.find((r) => r.id === nodeId);
        if (rep?.summary) setSummary(rep.summary);
      })
      .catch(() => setDetailsError(true));
  }, [memberIds, nodeId]);

  useEffect(() => {
    setDetails(null);
    setSummary(null);
    setSummaryError(null);
    loadDetails();
  }, [loadDetails]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  const rep = details?.find((d) => d.id === nodeId);
  const members = (details ?? []).filter((d) => d.id !== nodeId);
  const url = rep?.url ?? data.url;
  const createdAt = rep?.created_at;
  const matchedPackage = rep?.matched_package;
  const relevance = getRelevancePresentation(data.relevance_score);
  const categoryColor = CATEGORY_COLORS[data.category] ?? '#6B7280';

  const handleOpen = useCallback(
    (itemId: number, itemUrl: string, title: string, sourceType: string, score: number) => {
      // Context-engine click signal — the graph's pre-panel behavior, kept so
      // engagement telemetry still sees graph opens.
      cmd('record_interaction', { sourceItemId: itemId, action: 'click' }).catch(() => {});
      void recordInteraction(itemId, 'click', toFeedbackItem(title, sourceType, score));
      recordTrustEvent({
        eventType: 'acted_on',
        signalId: String(itemId),
        sourceType,
        topic: title,
        notes: 'open_link',
      });
      openExternal(itemUrl);
    },
    [recordInteraction],
  );

  const handleSave = useCallback(() => {
    void recordInteraction(nodeId, 'save', toFeedbackItem(data.title, data.source_type, data.relevance_score));
    recordTrustEvent({
      eventType: 'acted_on',
      signalId: String(nodeId),
      sourceType: data.source_type,
      topic: data.title,
      notes: 'save',
    });
  }, [nodeId, data.title, data.source_type, data.relevance_score, recordInteraction]);

  const handleSnooze = useCallback(() => {
    void recordInteraction(nodeId, 'snooze', toFeedbackItem(data.title, data.source_type, data.relevance_score));
    cmd('snooze_item', { sourceItemId: nodeId, days: 7 }).catch(() => {});
    recordTrustEvent({
      eventType: 'dismissed',
      signalId: String(nodeId),
      sourceType: data.source_type,
      topic: data.title,
      notes: 'snoozed_7d',
    });
  }, [nodeId, data.title, data.source_type, data.relevance_score, recordInteraction]);

  const handleNotRelevant = useCallback(() => {
    void recordInteraction(nodeId, 'mark_irrelevant', toFeedbackItem(data.title, data.source_type, data.relevance_score));
    recordTrustEvent({
      eventType: 'false_positive',
      signalId: String(nodeId),
      sourceType: data.source_type,
      topic: data.title,
      notes: 'mark_irrelevant',
    });
  }, [nodeId, data.title, data.source_type, data.relevance_score, recordInteraction]);

  const handleGenerateSummary = useCallback(() => {
    setSummaryLoading(true);
    setSummaryError(null);
    cmd('generate_item_summary', { itemId: nodeId })
      .then((r) => setSummary(r.summary))
      .catch((e) => setSummaryError(String(e)))
      .finally(() => setSummaryLoading(false));
  }, [nodeId]);

  return (
    <div
      role="complementary"
      aria-label={t('signals.graphDetailAria')}
      className="absolute inset-y-0 end-0 z-20 w-[320px] flex flex-col border-s overflow-hidden"
      style={{ backgroundColor: 'var(--color-bg-secondary)', borderColor: 'var(--color-border)' }}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b shrink-0" style={{ borderColor: 'var(--color-border)' }}>
        <div className="flex items-center gap-2 min-w-0">
          <span
            className="inline-block w-2.5 h-2.5 rounded-full shrink-0"
            style={{ backgroundColor: categoryColor }}
            aria-hidden="true"
          />
          <span className="text-[11px] font-medium text-text-secondary truncate">
            {t(`signals.graphCat_${data.category}`, data.category)}
          </span>
          {data.affects_you && (
            <span
              className="px-1.5 py-0.5 text-[10px] rounded border shrink-0"
              style={{ color: AFFECTS_GOLD, borderColor: `${AFFECTS_GOLD}66`, backgroundColor: `${AFFECTS_GOLD}1A` }}
            >
              {t('signals.graphAffectsYou')}
            </span>
          )}
        </div>
        <button
          onClick={onClose}
          aria-label={t('action.close')}
          className="text-[11px] text-text-muted hover:text-text-secondary transition-colors ms-2 shrink-0"
        >
          {t('action.close')}
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-3 py-3">
        {/* Title + meta */}
        <h3 className="text-sm font-semibold text-text-primary leading-snug mb-1.5">{data.title}</h3>
        <div className="flex items-center gap-2 flex-wrap mb-3 text-[10px] text-text-muted">
          <span>{getSourceLabel(data.source_type)}</span>
          {createdAt && <span>{formatTimeAgo(createdAt)}</span>}
          <span className={`uppercase tracking-wider ${relevance.colorClass}`}>{t(relevance.labelKey)}</span>
        </div>

        {/* Dependency grounding — why the gold ring */}
        {matchedPackage && (
          <div className="mb-3">
            <span
              className="px-1.5 py-0.5 text-[10px] rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/30"
              title={t('signals.groundedIn')}
            >
              🎯 {matchedPackage}
            </span>
          </div>
        )}

        {/* Actions */}
        <div className="flex items-center gap-1.5 flex-wrap mb-3" role="group" aria-label={t('feedback.actions')}>
          {url && isSafeUrl(url) && (
            <button
              onClick={() => handleOpen(nodeId, url, data.title, data.source_type, data.relevance_score)}
              className="px-2.5 py-1 text-[11px] bg-accent-primary text-bg-primary rounded hover:bg-text-secondary transition-colors font-medium"
            >
              {t('feedback.openLink')}
            </button>
          )}
          <button
            onClick={handleSave}
            disabled={!!feedback}
            className={`px-2.5 py-1 text-[11px] rounded font-medium transition-colors ${
              feedback === 'save'
                ? 'bg-success/20 text-success cursor-default'
                : feedback
                ? 'bg-bg-tertiary text-text-muted cursor-not-allowed'
                : 'bg-success/20 text-success hover:bg-success/30'
            }`}
          >
            {feedback === 'save' ? `✓ ${t('feedback.saved')}` : t('action.save')}
          </button>
          <button
            onClick={handleSnooze}
            disabled={!!feedback}
            className={`px-2.5 py-1 text-[11px] rounded font-medium transition-colors ${
              feedback === 'snooze'
                ? 'bg-amber-500/20 text-amber-400 cursor-default'
                : feedback
                ? 'bg-bg-tertiary text-text-muted cursor-not-allowed'
                : 'bg-amber-500/10 text-amber-400/80 hover:bg-amber-500/20 hover:text-amber-400'
            }`}
          >
            {feedback === 'snooze' ? `⏰ ${t('feedback.snoozed')}` : t('action.snooze')}
          </button>
          <button
            onClick={handleNotRelevant}
            disabled={!!feedback}
            className={`px-2.5 py-1 text-[11px] rounded font-medium transition-colors ${
              feedback === 'mark_irrelevant'
                ? 'bg-error/20 text-error cursor-default'
                : feedback
                ? 'bg-bg-tertiary text-text-muted cursor-not-allowed'
                : 'bg-error/10 text-error/80 hover:bg-error/20 hover:text-error'
            }`}
          >
            {feedback === 'mark_irrelevant' ? `⊘ ${t('feedback.marked')}` : t('feedback.notRelevant')}
          </button>
        </div>

        {/* AI Summary — cached shows instantly, otherwise on demand */}
        <div className="mb-3">
          {summary ? (
            <div className="p-2 bg-bg-primary/50 rounded border border-cyan-500/20">
              <div className="text-[10px] text-cyan-400 font-medium mb-1">{t('results.aiSummary')}</div>
              <div className="text-xs text-text-secondary leading-relaxed">{summary}</div>
            </div>
          ) : (
            <button
              onClick={handleGenerateSummary}
              disabled={summaryLoading}
              className="text-[11px] px-2.5 py-1.5 rounded border border-cyan-500/20 text-cyan-400 hover:bg-cyan-500/10 transition-colors disabled:opacity-50"
            >
              {summaryLoading ? t('briefing.generating') : t('results.generateAiSummary')}
            </button>
          )}
          {summaryError && <div className="mt-1 text-[10px] text-red-400">{summaryError}</div>}
        </div>

        {/* Story members — every collapsed item, each openable */}
        {(data.member_count ?? 1) > 1 && (
          <div className="border-t pt-3" style={{ borderColor: 'var(--color-border)' }}>
            <div className="text-[11px] font-semibold text-text-secondary mb-2">
              {t('signals.graphStoryMembers', { count: (data.member_count ?? 1) - 1 })}
            </div>
            {details === null && !detailsError && (
              <div className="text-[11px] text-text-muted">{t('action.loading')}</div>
            )}
            {detailsError && (
              <div className="text-[11px] text-red-400">
                {t('signals.graphDetailError')}
                <button onClick={loadDetails} className="ms-2 underline text-red-300 hover:text-red-200">
                  {t('action.retry')}
                </button>
              </div>
            )}
            <ul className="space-y-2">
              {members.map((m) => (
                <li key={m.id} className="p-2 rounded border bg-bg-primary/40" style={{ borderColor: 'var(--color-border)' }}>
                  <div className="text-[11px] text-text-primary leading-snug mb-1">{m.title}</div>
                  <div className="flex items-center gap-2 text-[10px] text-text-muted">
                    <span>{getSourceLabel(m.source_type)}</span>
                    <span>{formatTimeAgo(m.created_at)}</span>
                    {m.matched_package && (
                      <span className="text-emerald-400" title={t('signals.groundedIn')}>
                        🎯 {m.matched_package}
                      </span>
                    )}
                    {m.url && isSafeUrl(m.url) && (
                      <button
                        onClick={() => handleOpen(m.id, m.url!, m.title, m.source_type, m.relevance_score)}
                        className="ms-auto text-text-secondary hover:text-text-primary transition-colors underline"
                      >
                        {t('feedback.openLink')}
                      </button>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </div>
  );
}
