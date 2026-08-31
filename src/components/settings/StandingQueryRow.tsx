// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useCallback, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import type { StandingQuery, StandingQueryMatch } from '../../lib/commands';

interface StandingQueryRowProps {
  query: StandingQuery;
  onDelete: (id: number) => void;
}

/**
 * One standing query in the Settings list: query text, extracted keywords,
 * match counts from the evaluator, a lazy-loaded "recent matches" drawer
 * (get_standing_query_matches, first expand only), and a two-step inline
 * delete confirmation (same idiom as TeamSection's leave-team button).
 */
export const StandingQueryRow = memo(function StandingQueryRow({ query, onDelete }: StandingQueryRowProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [matches, setMatches] = useState<StandingQueryMatch[]>([]);
  const [matchesLoading, setMatchesLoading] = useState(false);
  const [matchesError, setMatchesError] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const fetchedRef = useRef(false);

  const toggleExpand = useCallback(() => {
    const next = !expanded;
    setExpanded(next);
    if (next && !fetchedRef.current) {
      fetchedRef.current = true;
      setMatchesLoading(true);
      void cmd('get_standing_query_matches', { id: query.id, limit: 5 })
        .then((rows) => setMatches(Array.isArray(rows) ? rows : []))
        .catch(() => setMatchesError(true))
        .finally(() => setMatchesLoading(false));
    }
  }, [expanded, query.id]);

  return (
    <li className="bg-bg-secondary rounded-lg border border-border px-3 py-2.5">
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={toggleExpand}
          aria-expanded={expanded}
          aria-label={expanded
            ? t('settings.standingQueries.hideMatches', { query: query.query_text })
            : t('settings.standingQueries.showMatches', { query: query.query_text })}
          className="flex items-center gap-2 min-w-0 flex-1 text-start group"
        >
          <span className="text-text-muted text-xs shrink-0" aria-hidden="true">{expanded ? '▾' : '▸'}</span>
          <span className="min-w-0">
            <span className="block text-xs font-medium text-text-primary truncate group-hover:text-text-primary">
              {query.query_text}
            </span>
            {query.keywords.length > 0 && (
              <span className="block text-[10px] text-text-muted font-mono truncate mt-0.5">
                {query.keywords.join(' · ')}
              </span>
            )}
          </span>
        </button>

        <span className="shrink-0 text-end">
          {query.new_matches > 0 && (
            <span className="inline-block px-1.5 py-0.5 text-[10px] rounded bg-accent-gold/10 text-accent-gold border border-accent-gold/20 tabular-nums me-1.5">
              {t('settings.standingQueries.newMatches', { count: query.new_matches })}
            </span>
          )}
          {query.total_matches > 0 ? (
            <span className="text-[10px] text-text-muted tabular-nums">
              {t('settings.standingQueries.totalMatches', { count: query.total_matches })}
            </span>
          ) : query.last_run === null ? (
            <span className="text-[10px] text-text-muted">{t('settings.standingQueries.neverRun')}</span>
          ) : (
            <span className="text-[10px] text-text-muted">{t('settings.standingQueries.noMatchesYet')}</span>
          )}
        </span>

        {!confirming ? (
          <button
            type="button"
            onClick={() => setConfirming(true)}
            aria-label={t('settings.standingQueries.delete', { query: query.query_text })}
            className="shrink-0 px-2 py-1 text-[11px] text-text-muted border border-border rounded-md hover:border-error/30 hover:text-error transition-colors"
          >
            {t('action.delete')}
          </button>
        ) : (
          <span className="shrink-0 flex items-center gap-1.5">
            <button
              type="button"
              onClick={() => onDelete(query.id)}
              aria-label={t('settings.standingQueries.confirmDelete', { query: query.query_text })}
              className="px-2 py-1 text-[11px] font-medium text-error border border-error/30 rounded-md bg-error/10 hover:bg-error/20 transition-colors"
            >
              {t('settings.standingQueries.confirmDeleteLabel')}
            </button>
            <button
              type="button"
              onClick={() => setConfirming(false)}
              aria-label={t('action.cancel')}
              className="px-2 py-1 text-[11px] text-text-muted border border-border rounded-md hover:text-text-primary transition-colors"
            >
              {t('action.cancel')}
            </button>
          </span>
        )}
      </div>

      {expanded && (
        <div className="mt-2 ms-5 pt-2 border-t border-border">
          <p className="text-[10px] uppercase tracking-wider text-text-muted mb-1.5">
            {t('settings.standingQueries.recentMatches')}
          </p>
          {matchesLoading ? (
            <p className="text-xs text-text-muted">{t('settings.standingQueries.matchesLoading')}</p>
          ) : matchesError ? (
            <p className="text-xs text-error">{t('settings.standingQueries.matchesError')}</p>
          ) : matches.length === 0 ? (
            <p className="text-xs text-text-muted">{t('settings.standingQueries.noMatchesYet')}</p>
          ) : (
            <ul className="space-y-1">
              {matches.map((m) => (
                <li key={m.item_id} className="flex items-center gap-2 min-w-0">
                  <span className="text-[10px] px-1.5 py-0.5 rounded bg-bg-tertiary text-text-muted shrink-0">
                    {m.source_type}
                  </span>
                  {m.url ? (
                    <a
                      href={m.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-xs text-text-secondary hover:text-text-primary truncate underline-offset-2 hover:underline"
                    >
                      {m.title}
                    </a>
                  ) : (
                    <span className="text-xs text-text-secondary truncate">{m.title}</span>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </li>
  );
});
