import { useMemo, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store';
import { getSourceLabel, getSourceColorClass, getSourcesByCategory, getSourceCategory } from '../config/sources';
import type { SourceRelevance } from '../types/analysis';
import { ResultItem } from './ResultItem';

const CATEGORY_META: Record<string, { label: string; description: string; color: string }> = {
  security: {
    label: 'Security',
    description: 'Vulnerabilities, advisories, and patches affecting your stack',
    color: 'text-red-400',
  },
  package_registry: {
    label: 'Dependencies',
    description: 'New versions, deprecations, and breaking changes in your packages',
    color: 'text-cyan-400',
  },
  news: {
    label: 'News',
    description: 'Industry news, releases, and trending discussions',
    color: 'text-orange-400',
  },
  research: {
    label: 'Research',
    description: 'Papers, models, and technical deep dives',
    color: 'text-purple-400',
  },
  community: {
    label: 'Community',
    description: 'Developer discussions, Q&A, and community signals',
    color: 'text-green-400',
  },
  social: {
    label: 'Social',
    description: 'Developer discourse from social platforms',
    color: 'text-blue-400',
  },
};

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

  const { relevanceResults, feedbackGiven } = useAppStore(
    useShallow(s => ({
      relevanceResults: s.appState.relevanceResults,
      feedbackGiven: s.feedbackGiven,
    })),
  );
  const recordInteraction = useAppStore(s => s.recordInteraction);

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
            {t('chapters.title', 'Source Chapters')}
          </h2>
          <p className="text-xs text-text-muted mt-1">
            {t('chapters.subtitle', 'Browse content by category — each chapter groups related sources together')}
          </p>
        </div>

        <div className="grid grid-cols-2 gap-3">
          {CATEGORY_ORDER.map(cat => {
            const meta = CATEGORY_META[cat];
            if (meta == null) return null;
            const items = chapters.get(cat) ?? [];
            const relevantCount = items.filter(i => i.relevant).length;
            const sources = getSourcesByCategory().get(cat) ?? [];
            const criticalCount = items.filter(i => i.signal_priority === 'critical' || i.signal_priority === 'alert').length;

            return (
              <button
                key={cat}
                onClick={() => { setActiveChapter(cat); }}
                className="bg-bg-secondary border border-border rounded-lg p-4 text-left hover:border-border/80 hover:bg-bg-tertiary/50 transition-all group"
              >
                <div className="flex items-center justify-between mb-2">
                  <span className={`text-sm font-medium ${meta.color}`}>
                    {meta.label}
                  </span>
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
                  {sources.slice(0, 6).map(id => (
                    <span
                      key={id}
                      className={`px-1.5 py-0.5 text-[10px] rounded ${getSourceColorClass(id)}`}
                    >
                      {getSourceLabel(id)}
                    </span>
                  ))}
                </div>
              </button>
            );
          })}
        </div>
      </div>
    );
  }

  // Chapter detail — show items from selected category
  const meta = CATEGORY_META[activeChapter];
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
            {items.length} {t('chapters.items', 'items')} {t('chapters.from', 'from')} {sources.map(s => getSourceLabel(s)).join(', ')}
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
          {t('chapters.empty', 'No items from these sources yet. They will appear after the next analysis cycle.')}
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
              {t('chapters.showingFirst', 'Showing first 50 of {{total}} items', { total: items.length })}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
