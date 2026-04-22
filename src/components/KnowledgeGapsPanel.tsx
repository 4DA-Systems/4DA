// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { ProGate } from './ProGate';
import { useTranslatedContent } from './ContentTranslationProvider';
import type { EvidenceItem } from '../../src-tauri/bindings/bindings/EvidenceItem';
import type { Urgency } from '../../src-tauri/bindings/bindings/Urgency';

// Phase 5 (2026-04-17): consumes canonical EvidenceFeed of kind=Gap items.
// The 4-severity legacy scale is mapped 1:1 to the shared Urgency scale
// (critical/high/medium/watch).

const URGENCY_CONFIG: Record<Urgency, { color: string; bg: string; border: string }> = {
  critical: { color: 'text-red-400', bg: 'bg-red-500/10', border: 'border-red-500/20' },
  high: { color: 'text-amber-400', bg: 'bg-amber-500/10', border: 'border-amber-500/20' },
  medium: { color: 'text-yellow-400', bg: 'bg-yellow-500/10', border: 'border-yellow-500/20' },
  watch: { color: 'text-text-secondary', bg: 'bg-gray-500/10', border: 'border-gray-500/20' },
};

/** Extract the displayed dependency name from an EvidenceItem. Prefer
 * affected_deps[0] (set by the materializer for gap items); fall back to
 * title stripped of its "Knowledge gap: " prefix. */
function depNameFromItem(item: EvidenceItem): string {
  if (item.affected_deps.length > 0) return item.affected_deps[0]!;
  return item.title.replace(/^Knowledge gap:\s*/, '');
}

export const KnowledgeGapsPanel = memo(function KnowledgeGapsPanel() {
  const { t } = useTranslation();
  const { getTranslated } = useTranslatedContent();
  const [items, setItems] = useState<EvidenceItem[]>([]);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    const load = async () => {
      try {
        const feed = await cmd('get_knowledge_gaps');
        setItems(feed.items);
      } catch {
        // Knowledge gaps are optional
      }
    };
    load();
  }, []);

  const urgentCount = items.filter(
    (it) => it.urgency === 'critical' || it.urgency === 'high',
  ).length;

  return (
    <ProGate feature={t('knowledgeGaps.feature')}>
    <div className="mb-6 bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {items.length === 0 ? (
        <div className="px-5 py-4 flex items-center gap-3">
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
            <span className="text-text-secondary">✓</span>
          </div>
          <div>
            <h2 className="font-medium text-white text-sm">{t('knowledgeGaps.title')}</h2>
            <p className="text-xs text-text-muted">{t('knowledgeGaps.noGaps', 'No gaps detected — your knowledge is current')}</p>
          </div>
        </div>
      ) : (
      <>
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full px-5 py-4 flex items-center justify-between hover:bg-[#1A1A1A] transition-colors"
      >
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
            <span className="text-text-secondary">📖</span>
          </div>
          <div className="text-start">
            <h2 className="font-medium text-white text-sm">{t('knowledgeGaps.title')}</h2>
            <p className="text-xs text-text-muted">
              {t('knowledgeGaps.count', { count: items.length })}
              {urgentCount > 0 && <span className="text-amber-400 ms-1">{t('knowledgeGaps.needAttention', { count: urgentCount })}</span>}
            </p>
          </div>
        </div>
        <span className="text-text-muted text-sm">{expanded ? '▾' : '▸'}</span>
      </button>

      {expanded && (
        <div className="p-4 space-y-2 border-t border-border">
          {items.map((it) => {
            const cfg = URGENCY_CONFIG[it.urgency];
            const depName = depNameFromItem(it);
            const missedCount = it.evidence.filter(c => c.url !== null).length;
            const rawPath = it.affected_projects[0] ?? '';
            const projectPath = rawPath.split(/[/\\]/).filter(Boolean).pop() ?? rawPath;
            return (
              <div key={it.id} className={`px-4 py-3 rounded-lg border ${cfg.border} ${cfg.bg}`}>
                <div className="flex items-center gap-2">
                  <span className={`text-sm font-medium ${cfg.color}`}>{depName}</span>
                  <span className={`ms-auto text-[10px] px-1.5 py-0.5 rounded ${cfg.bg} ${cfg.color} border ${cfg.border}`}>
                    {it.urgency}
                  </span>
                </div>
                <p className="text-xs text-text-secondary mt-1">
                  {t('knowledgeGaps.missedArticles', { count: missedCount })}
                </p>
                {it.evidence.length > 0 && (() => {
                  const topCite = it.evidence[0]!;
                  const isSecurityTop = topCite.relevance_note.toLowerCase().includes('security');
                  const rest = it.evidence.slice(1, 4);
                  return (
                  <div className="mt-2 space-y-1">
                    <div className={`text-xs p-1.5 rounded ${isSecurityTop ? 'bg-red-500/10 border border-red-500/20' : 'bg-bg-tertiary/50'}`}>
                      <span className="text-[9px] text-text-muted uppercase tracking-wide">{t('knowledgeGaps.startHere', 'Start here')}</span>
                      <div className="mt-0.5">
                        {topCite.url ? (
                          <a href={topCite.url} target="_blank" rel="noopener noreferrer" className={`font-medium ${isSecurityTop ? 'text-red-400 hover:text-red-300' : 'text-text-primary hover:text-white'} transition-colors`}>
                            {getTranslated(`${it.id}_cite_0`, topCite.title)}
                          </a>
                        ) : (
                          <span className="font-medium text-text-primary">{getTranslated(`${it.id}_cite_0`, topCite.title)}</span>
                        )}
                      </div>
                    </div>
                    {rest.length > 0 && (
                      <details className="group">
                        <summary className="flex items-center gap-1 cursor-pointer select-none text-[10px] text-text-muted hover:text-text-secondary transition-colors list-none">
                          <span className="group-open:rotate-90 transition-transform">&#9654;</span>
                          {rest.length} more {rest.length === 1 ? 'article' : 'articles'}
                        </summary>
                        <div className="mt-1 space-y-1">
                          {rest.map((cite, i) => (
                            <div key={i + 1} className="text-[11px]">
                              {cite.url ? (
                                <a href={cite.url} target="_blank" rel="noopener noreferrer" className="text-text-secondary hover:text-white transition-colors">
                                  {getTranslated(`${it.id}_cite_${i + 1}`, cite.title)}
                                </a>
                              ) : (
                                <span className="text-text-secondary">{getTranslated(`${it.id}_cite_${i + 1}`, cite.title)}</span>
                              )}
                            </div>
                          ))}
                        </div>
                      </details>
                    )}
                  </div>
                  );
                })()}
                {it.suggested_actions.length > 0 && (
                  <div className="mt-2 flex flex-wrap gap-1.5">
                    {it.suggested_actions.map((action) => (
                      <span key={action.action_id} className={`text-[10px] px-2 py-0.5 rounded border ${cfg.border} ${cfg.color} cursor-default`} title={action.description}>
                        {action.label}
                      </span>
                    ))}
                  </div>
                )}
                <div className="text-[10px] text-text-muted mt-1">
                  {it.explanation} {projectPath && `· ${projectPath}`}
                </div>
              </div>
            );
          })}
        </div>
      )}
      </>
      )}
    </div>
    </ProGate>
  );
});
