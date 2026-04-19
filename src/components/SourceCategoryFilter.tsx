// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useCallback, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { getSourceLabel, getSourcesByCategory, getSourceColorClass } from '../config/sources';

const CATEGORY_ORDER = ['security', 'package_registry', 'news', 'social', 'research', 'community', 'general'];
const CATEGORY_LABELS: Record<string, string> = {
  security: 'Security',
  package_registry: 'Packages',
  news: 'News',
  social: 'Social',
  research: 'Research',
  community: 'Community',
  general: 'Other',
};
const CATEGORY_COLORS: Record<string, string> = {
  security: 'text-red-400',
  package_registry: 'text-cyan-400',
  news: 'text-orange-400',
  social: 'text-blue-400',
  research: 'text-purple-400',
  community: 'text-green-400',
  general: 'text-text-muted',
};

interface Props {
  sourceFilters: Set<string>;
  sourcesWithResults: Set<string>;
  onToggle: (id: string) => void;
  onReset: () => void;
}

export function SourceCategoryFilter({ sourceFilters, sourcesWithResults, onToggle, onReset }: Props) {
  const { t } = useTranslation();
  const [openCategory, setOpenCategory] = useState<string | null>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Close dropdown on outside click
  useEffect(() => {
    if (openCategory == null) return;
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current != null && !dropdownRef.current.contains(e.target as Node)) {
        setOpenCategory(null);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => { document.removeEventListener('mousedown', handler); };
  }, [openCategory]);

  const categories = getSourcesByCategory();
  const totalActive = sourceFilters.size;
  const totalSources = Array.from(categories.values()).flat().length;

  const toggleCategory = useCallback((cat: string, enabled: boolean) => {
    const sources = categories.get(cat) ?? [];
    for (const s of sources) {
      if (enabled !== sourceFilters.has(s)) {
        onToggle(s);
      }
    }
  }, [categories, sourceFilters, onToggle]);

  return (
    <div ref={dropdownRef} className="flex items-center gap-1.5 flex-wrap" role="group" aria-label="Source category filters">
      <span className="text-xs text-text-muted">{t('results.sources', 'Sources')}</span>
      {CATEGORY_ORDER.filter(cat => categories.has(cat)).map(cat => {
        const sources = categories.get(cat)!;
        const activeCount = sources.filter(s => sourceFilters.has(s)).length;
        const hasResults = sources.some(s => sourcesWithResults.has(s));
        const isOpen = openCategory === cat;

        return (
          <div key={cat} className="relative">
            <button
              onClick={() => { setOpenCategory(isOpen ? null : cat); }}
              className={`px-2 py-1 text-xs rounded-lg transition-all flex items-center gap-1 ${
                activeCount > 0
                  ? `${CATEGORY_COLORS[cat] ?? 'text-text-muted'} bg-bg-tertiary`
                  : hasResults
                  ? 'text-text-muted hover:text-text-secondary'
                  : 'text-text-muted/40'
              }`}
            >
              {CATEGORY_LABELS[cat] ?? cat}
              <span className="text-[10px] opacity-60">{activeCount}/{sources.length}</span>
            </button>

            {isOpen && (
              <div className="absolute z-50 top-full mt-1 left-0 bg-bg-secondary border border-border rounded-lg shadow-lg p-2 min-w-[180px]">
                <div className="text-[10px] text-text-muted uppercase tracking-wider mb-1.5 px-1">
                  {CATEGORY_LABELS[cat]}
                </div>
                {sources.map(id => (
                  <label
                    key={id}
                    className="flex items-center gap-2 px-1 py-1 rounded hover:bg-bg-tertiary cursor-pointer text-xs"
                  >
                    <input
                      type="checkbox"
                      checked={sourceFilters.has(id)}
                      onChange={() => { onToggle(id); }}
                      className="rounded border-border"
                    />
                    <span className={`w-1.5 h-1.5 rounded-full ${getSourceColorClass(id).split(' ')[0]?.replace('/20', '') ?? 'bg-gray-500'}`} />
                    <span className={sourcesWithResults.has(id) ? 'text-text-secondary' : 'text-text-muted/50'}>
                      {getSourceLabel(id)}
                    </span>
                  </label>
                ))}
                <div className="flex gap-1.5 mt-1.5 pt-1.5 border-t border-border">
                  <button
                    onClick={() => { toggleCategory(cat, true); }}
                    className="text-[10px] text-text-muted hover:text-text-secondary px-1"
                  >
                    All
                  </button>
                  <button
                    onClick={() => { toggleCategory(cat, false); }}
                    className="text-[10px] text-text-muted hover:text-text-secondary px-1"
                  >
                    None
                  </button>
                </div>
              </div>
            )}
          </div>
        );
      })}

      {totalActive < totalSources && (
        <button
          onClick={onReset}
          className="text-[10px] text-text-muted hover:text-orange-400 transition-colors"
        >
          {t('results.clearSourceFilters', 'Reset')}
        </button>
      )}
    </div>
  );
}
