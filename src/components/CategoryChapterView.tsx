import { useMemo, useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store';
import { getSourceLabel, getSourceColorClass, getSourcesByCategory, getSourceCategory } from '../config/sources';
import type { SourceRelevance } from '../types/analysis';
import type { SourceHealthStatus } from '../types';
import { ResultItem } from './ResultItem';

interface CategoryMeta { label: string; description: string; color: string }

interface ChapterHealth {
  healthy: number;
  error: number;
  idle: number;
  total: number;
}

function computeChapterHealth(
  categorySourceIds: string[],
  sourceHealth: SourceHealthStatus[],
): ChapterHealth {
  let healthy = 0;
  let error = 0;
  let idle = 0;
  for (const id of categorySourceIds) {
    const h = sourceHealth.find(x => x.source_type === id);
    if (h == null) {
      idle++;
    } else if (h.status === 'healthy') {
      healthy++;
    } else if (h.status === 'error' || h.status === 'circuit_open') {
      error++;
    } else {
      idle++;
    }
  }
  return { healthy, error, idle, total: categorySourceIds.length };
}

const CATEGORY_ORDER = ['security', 'package_registry', 'news', 'research', 'community', 'social'];

/**
 * Book-style chapter navigation — each source category is a "chapter"
 * with items cascading within. Users browse by category instead of
 * a flat mixed feed.
 */
export function CategoryChapterView() {
  const { t } = useTranslation();
  const [activeChapter, setActiveChapter] = useState<string | null>(null);
  const [expandedItem, setExpandedItem] = useState<number | null>(null);

  const categoryMeta = useMemo((): Record<string, CategoryMeta> => ({
    security: { label: t('chapters.cat.security'), description: t('chapters.cat.securityDesc'), color: 'text-red-400' },
    package_registry: { label: t('chapters.cat.dependencies'), description: t('chapters.cat.dependenciesDesc'), color: 'text-cyan-400' },
    news: { label: t('chapters.cat.news'), description: t('chapters.cat.newsDesc'), color: 'text-orange-400' },
    research: { label: t('chapters.cat.research'), description: t('chapters.cat.researchDesc'), color: 'text-purple-400' },
    community: { label: t('chapters.cat.community'), description: t('chapters.cat.communityDesc'), color: 'text-green-400' },
    social: { label: t('chapters.cat.social'), description: t('chapters.cat.socialDesc'), color: 'text-blue-400' },
  }), [t]);

  const { relevanceResults, feedbackGiven, sourceHealth } = useAppStore(
    useShallow(s => ({
      relevanceResults: s.appState.relevanceResults,
      feedbackGiven: s.feedbackGiven,
      sourceHealth: s.sourceHealth,
    })),
  );
  const recordInteraction = useAppStore(s => s.recordInteraction);
  const loadSourceHealth = useAppStore(s => s.loadSourceHealth);

  // Refresh source health whenever the chapter grid is viewed — cheap,
  // gives users live visibility into per-source reliability. Was the
  // missing piece for "sources are erratic, sometimes populated
  // sometimes not": now you can SEE which sources are idle or failing.
  useEffect(() => {
    void loadSourceHealth();
  }, [loadSourceHealth]);

  // Group items by category
  const chapters = useMemo(() => {
    const groups = new Map<string, SourceRelevance[]>();
    for (const cat of CATEGORY_ORDER) {
      groups.set(cat, []);
    }
    for (const item of relevanceResults) {
      const cat = getSourceCategory(item.source_type ?? 'hackernews');
      const list = groups.get(cat);
      if (list != null) {
        list.push(item);
      } else {
        // Unknown category — put in community
        const fallback = groups.get('community') ?? [];
        fallback.push(item);
        groups.set('community', fallback);
      }
    }
    // Sort each chapter by score descending
    for (const [, items] of groups) {
      items.sort((a, b) => b.top_score - a.top_score);
    }
    return groups;
  }, [relevanceResults]);

  const handleToggleExpand = useCallback((id: number) => {
    setExpandedItem(prev => prev === id ? null : id);
  }, []);

  // Chapter overview — show all categories as cards
  if (activeChapter == null) {
    return (
      <div className="px-6 py-4">
        <div className="mb-6">
          <h2 className="text-lg font-medium text-text-primary">
            {t('chapters.title')}
          </h2>
          <p className="text-xs text-text-muted mt-1">
            {t('chapters.subtitle')}
          </p>
        </div>

        <div className="grid grid-cols-2 gap-3">
          {CATEGORY_ORDER.map(cat => {
            const meta = categoryMeta[cat];
            if (meta == null) return null;
            const items = chapters.get(cat) ?? [];
            const relevantCount = items.filter(i => i.relevant).length;
            const sources = getSourcesByCategory().get(cat) ?? [];
            const criticalCount = items.filter(i => i.signal_priority === 'critical' || i.signal_priority === 'alert').length;
            const health = computeChapterHealth(sources, sourceHealth);
            // Derive a single status color for the dot: red if any source
            // is failing, amber if all sources are idle/unknown, green if
            // at least one is healthy. This answers "are my sources
            // actually working?" at a glance.
            const statusColor = health.error > 0 ? 'bg-red-400' :
              health.healthy > 0 ? 'bg-green-400' :
              health.total > 0 ? 'bg-amber-400/60' :
              'bg-text-muted/30';
            const statusTitle = health.error > 0
              ? t('chapters.health.failing', '{{count}} of {{total}} sources failing', { count: health.error, total: health.total })
              : health.healthy > 0
                ? t('chapters.health.healthy', '{{healthy}} of {{total}} sources active', { healthy: health.healthy, total: health.total })
                : t('chapters.health.idle', 'Sources idle — waiting on next fetch');

            return (
              <button
                key={cat}
                onClick={() => { setActiveChapter(cat); }}
                className="bg-bg-secondary border border-border rounded-lg p-4 text-left hover:border-border/80 hover:bg-bg-tertiary/50 transition-all group"
              >
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-1.5">
                    <span
                      className={`w-1.5 h-1.5 rounded-full ${statusColor}`}
                      title={statusTitle}
                      aria-label={statusTitle}
                    />
                    <span className={`text-sm font-medium ${meta.color}`}>
                      {meta.label}
                    </span>
                  </div>
                  <div className="flex items-center gap-1.5">
                    {criticalCount > 0 && (
                      <span className="px-1.5 py-0.5 text-[10px] bg-red-500/20 text-red-400 rounded">
                        {criticalCount}
                      </span>
                    )}
                    <span className="text-xs text-text-muted">
                      {relevantCount}/{items.length}
                    </span>
                  </div>
                </div>
                <p className="text-[11px] text-text-muted leading-relaxed mb-3">
                  {meta.description}
                </p>
                <div className="flex flex-wrap gap-1">
                  {sources.slice(0, 6).map(id => {
                    const h = sourceHealth.find(x => x.source_type === id);
                    const sourceOk = h == null || h.status === 'healthy';
                    return (
                      <span
                        key={id}
                        className={`px-1.5 py-0.5 text-[10px] rounded ${getSourceColorClass(id)} ${sourceOk ? '' : 'opacity-40 line-through decoration-1'}`}
                        title={
                          h == null ? t('chapters.health.sourceIdle', '{{source}}: no recent fetches', { source: getSourceLabel(id) })
                          : h.status === 'healthy' ? t('chapters.health.sourceHealthy', '{{source}}: {{items}} items, last {{when}}', { source: getSourceLabel(id), items: h.items_fetched, when: h.last_success_relative ?? 'unknown' })
                          : t('chapters.health.sourceFailing', '{{source}}: {{message}}', { source: getSourceLabel(id), message: h.gap_message ?? h.status })
                        }
                      >
                        {getSourceLabel(id)}
                      </span>
                    );
                  })}
                </div>
              </button>
            );
          })}
        </div>
      </div>
    );
  }

  // Chapter detail — show items from selected category
  const meta = categoryMeta[activeChapter];
  const items = chapters.get(activeChapter) ?? [];
  const sources = getSourcesByCategory().get(activeChapter) ?? [];

  return (
    <div className="px-6 py-4">
      {/* Chapter header with back button */}
      <div className="flex items-center gap-3 mb-4">
        <button
          onClick={() => { setActiveChapter(null); }}
          className="text-text-muted hover:text-text-secondary transition-colors"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
          </svg>
        </button>
        <div>
          <h2 className={`text-base font-medium ${meta?.color ?? 'text-text-primary'}`}>
            {meta?.label ?? activeChapter}
          </h2>
          <p className="text-[11px] text-text-muted">
            {t('chapters.itemCount', '{{count}} items from {{sources}}', { count: items.length, sources: sources.map(s => getSourceLabel(s)).join(', ') })}
          </p>
        </div>
      </div>

      {/* Source badges */}
      <div className="flex gap-1.5 mb-4">
        {sources.map(id => (
          <span
            key={id}
            className={`px-2 py-0.5 text-[10px] rounded ${getSourceColorClass(id)}`}
          >
            {getSourceLabel(id)}
          </span>
        ))}
      </div>

      {/* Items cascade */}
      {items.length === 0 ? (
        <div className="text-center py-12 text-text-muted text-sm">
          {t('chapters.empty')}
        </div>
      ) : (
        <div className="space-y-1">
          {items.slice(0, 50).map(item => (
            <ResultItem
              key={item.id}
              item={item}
              isExpanded={expandedItem === item.id}
              onToggleExpand={handleToggleExpand}
              feedbackGiven={feedbackGiven}
              onRecordInteraction={recordInteraction}
            />
          ))}
          {items.length > 50 && (
            <p className="text-xs text-text-muted text-center py-2">
              {t('chapters.showingFirst', { total: items.length })}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
