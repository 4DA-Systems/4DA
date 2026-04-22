// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';
import type { SourceRelevance, FeedbackAction } from '../../types';
import { formatScore, getScoreColor } from '../../utils/score';
import { ArticleReader } from '../ArticleReader';
import { ScoreAutopsy } from '../ScoreAutopsy';
import { FeedbackButtons } from './FeedbackButtons';
import { SecurityTriageButtons } from './SecurityTriageButtons';

function truncateSourcePath(path: string): string {
  const parts = path.replace(/\\/g, '/').split('/').filter(Boolean);
  if (parts.length <= 2) return path;
  return parts.slice(-2).join('/');
}

function formatTimeAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const mins = Math.floor(diff / 60_000);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

interface ResultItemExpandedProps {
  item: SourceRelevance;
  feedback: FeedbackAction | undefined;
  onRecordInteraction: (itemId: number, actionType: FeedbackAction, item: SourceRelevance) => void;
  summary: string | null;
  summaryLoading: boolean;
  summaryError: string | null;
  onGenerateSummary: () => void;
}

export function ResultItemExpanded({
  item,
  feedback,
  onRecordInteraction,
  summary,
  summaryLoading,
  summaryError,
  onGenerateSummary,
}: ResultItemExpandedProps) {
  const { t } = useTranslation();
  return (
    <div id={`result-detail-${item.id}`} className="px-4 pb-3 border-t border-border/50 mt-2 pt-3">
      {/* Urgency + Category Badges */}
      {(item.score_breakdown?.necessity_urgency || item.score_breakdown?.necessity_category) && (
        <div className="flex items-center gap-1.5 mb-2">
          {item.score_breakdown?.necessity_urgency && item.score_breakdown.necessity_urgency !== 'none' && (
            <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
              item.score_breakdown.necessity_urgency === 'immediate' ? 'bg-red-500/15 text-red-400'
              : item.score_breakdown.necessity_urgency === 'this_week' ? 'bg-amber-500/15 text-amber-400'
              : 'bg-blue-500/15 text-blue-400'
            }`}>
              {item.score_breakdown.necessity_urgency === 'immediate' ? t('urgency.immediate', 'Immediate')
              : item.score_breakdown.necessity_urgency === 'this_week' ? t('urgency.thisWeek', 'This week')
              : t('urgency.awareness', 'Awareness')}
            </span>
          )}
          {item.score_breakdown?.necessity_category && item.score_breakdown.necessity_category !== 'none' && (
            <span className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-zinc-500/15 text-text-secondary">
              {item.score_breakdown.necessity_category === 'security_vulnerability' ? t('category.security', 'Security')
              : item.score_breakdown.necessity_category === 'breaking_change' ? t('category.breaking', 'Breaking Change')
              : item.score_breakdown.necessity_category === 'deprecation_notice' ? t('category.deprecation', 'Deprecation')
              : item.score_breakdown.necessity_category === 'blind_spot' ? t('category.blindSpot', 'Blind Spot')
              : t('category.decision', 'Decision Relevant')}
            </span>
          )}
        </div>
      )}

      {/* Necessity Reason */}
      {item.score_breakdown?.necessity_reason && (
        <p className="text-xs text-text-muted mb-2 italic">
          {item.score_breakdown.necessity_reason}
        </p>
      )}

      {/* Advisory Evidence Section (security items only) */}
      {item.score_breakdown?.necessity_category === 'security_vulnerability' && (
        <div className="bg-red-500/5 border border-red-500/20 rounded-lg p-3 mb-3">
          <div className="flex items-center gap-2 flex-wrap mb-2">
            {item.advisory_id && (
              <a
                href={item.url ?? '#'}
                target="_blank"
                rel="noopener noreferrer"
                className="px-2 py-0.5 rounded text-xs font-mono bg-red-500/15 text-red-400 hover:bg-red-500/25 transition-colors"
              >
                {item.advisory_id}
              </a>
            )}
            {item.score_breakdown?.advisory_source && (
              <span className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-emerald-500/15 text-emerald-400">
                {item.score_breakdown.advisory_source}
              </span>
            )}
            {item.score_breakdown?.cvss_score != null && (
              <span className={`px-1.5 py-0.5 rounded text-[10px] font-semibold ${
                item.score_breakdown.cvss_score >= 9.0 ? 'bg-red-500/20 text-red-400'
                : item.score_breakdown.cvss_score >= 7.0 ? 'bg-orange-500/20 text-orange-400'
                : item.score_breakdown.cvss_score >= 4.0 ? 'bg-yellow-500/20 text-yellow-400'
                : 'bg-zinc-500/20 text-text-muted'
              }`}>
                CVSS {item.score_breakdown.cvss_score.toFixed(1)}
              </span>
            )}
            {item.applicability && (
              <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
                item.applicability === 'affected' ? 'bg-red-500/20 text-red-400'
                : item.applicability === 'likely_affected' ? 'bg-orange-500/20 text-orange-400'
                : 'bg-yellow-500/20 text-yellow-400'
              }`}>
                {item.applicability === 'affected' ? t('results.affected', 'Affected')
                : item.applicability === 'likely_affected' ? t('results.likelyAffected', 'Likely Affected')
                : t('results.needsVerification', 'Needs Verification')}
              </span>
            )}
          </div>
          {item.score_breakdown?.is_version_affected === true && (
            <div className="text-[10px] text-red-400 font-medium mb-1.5">
              {t('results.versionAffected', 'Your installed version is in the affected range')}
            </div>
          )}
          {item.score_breakdown?.is_version_affected === false && (
            <div className="text-[10px] text-emerald-400 font-medium mb-1.5">
              {t('results.versionNotAffected', 'Your installed version is not affected')}
            </div>
          )}
          {item.score_breakdown?.matched_deps && item.score_breakdown.matched_deps.length > 0 && (
            <div className="text-xs text-text-secondary space-y-1">
              {item.score_breakdown.matched_deps.map((dep, i) => (
                <div key={i} className="flex items-center gap-2">
                  <span className="font-mono text-text-primary">{dep}</span>
                  {i === 0 && item.score_breakdown?.installed_version && (
                    <span className={`font-mono ${item.score_breakdown?.is_version_affected ? 'text-red-400' : 'text-text-muted'}`}>
                      v{item.score_breakdown.installed_version}
                    </span>
                  )}
                  {i === 0 && item.score_breakdown?.fixed_version && (
                    <span className="text-emerald-400 font-mono">&rarr; v{item.score_breakdown.fixed_version}</span>
                  )}
                  {i === 0 && item.score_breakdown?.dependency_path && (
                    <span className="text-text-muted">({item.score_breakdown.dependency_path})</span>
                  )}
                </div>
              ))}
            </div>
          )}
          {item.created_at && (
            <div className="text-[10px] text-text-muted mt-1.5">
              {t('results.published', 'Published')} {formatTimeAgo(item.created_at)}
            </div>
          )}
        </div>
      )}

      {/* Why This Matters - Full Display */}
      {item.explanation && !item.explanation.includes('No judgment provided by LLM') && (
        <div className="mb-3 p-2 bg-bg-primary/50 rounded border border-accent-gold/30">
          <div className="text-xs text-accent-gold font-medium mb-1">
            {t('results.whyThisMatters')}
          </div>
          <div className="text-xs text-text-secondary leading-relaxed">
            {item.explanation.replace(/^Filtered:\s*/i, '')}
          </div>
        </div>
      )}

      {/* Feedback / Triage Buttons */}
      {item.score_breakdown?.necessity_category === 'security_vulnerability' ? (
        <SecurityTriageButtons item={item} />
      ) : (
        <FeedbackButtons
          item={item}
          feedback={feedback}
          onRecordInteraction={onRecordInteraction}
        />
      )}

      {/* AI Summary */}
      <div className="mb-3">
        {summary ? (
          <div className="p-2 bg-bg-primary/50 rounded border border-cyan-500/20">
            <div className="text-[10px] text-cyan-400 font-medium mb-1">{t('results.aiSummary')}</div>
            <div className="text-xs text-text-secondary leading-relaxed">{summary}</div>
          </div>
        ) : (
          <button
            onClick={onGenerateSummary}
            disabled={summaryLoading}
            aria-label={summaryLoading ? t('briefing.generating') : `${t('results.generateAiSummary')} for ${item.title}`}
            className="text-[11px] px-2.5 py-1.5 rounded border border-cyan-500/20 text-cyan-400 hover:bg-cyan-500/10 transition-colors disabled:opacity-50"
          >
            {summaryLoading ? t('briefing.generating') : t('results.generateAiSummary')}
          </button>
        )}
        {summaryError && (
          <div className="mt-1 text-[10px] text-red-400">{summaryError}</div>
        )}
      </div>

      {/* Article Reader */}
      <ArticleReader
        itemId={item.id}
        url={item.url ?? undefined}
        contentType={item.score_breakdown?.content_type}
      />

      {/* Context Matches — top match inline, rest collapsible */}
      {item.matches.length > 0 && (() => {
        const topMatch = item.matches[0]!;
        return (
        <div className="mb-3">
          <div className="text-xs text-text-muted mb-1.5">
            {t('results.contextMatches', { count: item.matches.length })}
          </div>
          <div className="text-xs bg-bg-primary rounded p-2 border border-border/30">
            <div className="flex items-center gap-2">
              <span className={`font-mono ${getScoreColor(topMatch.similarity)}`}>
                {formatScore(topMatch.similarity)}
              </span>
              <span className="text-text-muted">&rarr;</span>
              <span className="text-accent-gold font-medium truncate">
                {truncateSourcePath(topMatch.source_file)}
              </span>
            </div>
          </div>
          {item.matches.length > 1 && (
            <details className="group mt-1">
              <summary className="flex items-center gap-1 cursor-pointer select-none text-[10px] text-text-muted hover:text-text-secondary transition-colors list-none">
                <span className="group-open:rotate-90 transition-transform">&#9654;</span>
                {item.matches.length - 1} more
              </summary>
              <ul className="mt-1 space-y-1">
                {item.matches.slice(1).map((match, i) => (
                  <li key={i} className="text-xs bg-bg-primary rounded p-2 border border-border/30">
                    <div className="flex items-center gap-2">
                      <span className={`font-mono ${getScoreColor(match.similarity)}`}>
                        {formatScore(match.similarity)}
                      </span>
                      <span className="text-text-muted">&rarr;</span>
                      <span className="text-accent-gold font-medium truncate">
                        {truncateSourcePath(match.source_file)}
                      </span>
                    </div>
                  </li>
                ))}
              </ul>
            </details>
          )}
        </div>
        );
      })()}

      {/* Score Details (Tier 3 — on demand) */}
      <details className="group">
        <summary className="flex items-center gap-2 cursor-pointer select-none text-xs text-text-muted hover:text-text-secondary transition-colors list-none">
          <span className="text-[10px] group-open:rotate-90 transition-transform">&#9654;</span>
          {t('results.scoreDetails', 'Score details')}
        </summary>
        <div className="mt-2">
          <ScoreAutopsy
            itemId={item.id}
            sourceType={item.source_type || 'hackernews'}
            currentScore={item.top_score}
          />
        </div>
      </details>
    </div>
  );
}
