// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useLicense } from '../../hooks/use-license';
import { cmd } from '../../lib/commands';
import { isSignalGateError, translateError } from '../../utils/error-messages';
import { SignalUpgradeCTA } from '../SignalUpgradeCTA';
import { StandingQueryRow } from './StandingQueryRow';
import type { StandingQuery, StandingQuerySuggestion } from '../../lib/commands';

/** Backend caps active queries at 10 (standing_queries.rs::create_standing_query). */
const MAX_ACTIVE_QUERIES = 10;

/**
 * Map the backend's known create-rejections to specific, actionable copy;
 * everything else goes through the shared translateError patterns.
 */
function describeCreateError(e: unknown, t: (key: string) => string): string {
  const raw = e instanceof Error ? e.message : String(e ?? '');
  if (/maximum of 10/i.test(raw)) return t('settings.standingQueries.maxReached');
  if (/no meaningful keywords|cannot be empty/i.test(raw)) return t('settings.standingQueries.errorNoKeywords');
  return translateError(e);
}

/**
 * Standing Queries management — Settings > Intelligence.
 *
 * Signal-gated (same pattern as SignalsPanel / BlindSpotsPaywall): functional
 * when licensed, the standard locked state with SignalUpgradeCTA otherwise.
 * The backend independently enforces the gate on every command; a gate
 * rejection from the backend routes to the same locked state via the shared
 * isSignalGateError classifier — a paywall is never rendered as a fault.
 */
export const StandingQueriesSection = memo(function StandingQueriesSection() {
  const { t } = useTranslation();
  const { isPro } = useLicense();
  const [queries, setQueries] = useState<StandingQuery[]>([]);
  const [suggestions, setSuggestions] = useState<StandingQuerySuggestion[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  const [gated, setGated] = useState(false);
  const [queryText, setQueryText] = useState('');
  const [creating, setCreating] = useState(false);
  const [creatingTopic, setCreatingTopic] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const rows = await cmd('list_standing_queries');
    setQueries(Array.isArray(rows) ? rows : []);
  }, []);

  useEffect(() => {
    if (!isPro) {
      setLoading(false);
      return;
    }
    let alive = true;
    void (async () => {
      try {
        const rows = await cmd('list_standing_queries');
        if (alive) setQueries(Array.isArray(rows) ? rows : []);
      } catch (e) {
        if (!alive) return;
        if (isSignalGateError(e)) setGated(true);
        else setLoadError(true);
      }
      try {
        const sugg = await cmd('get_standing_query_suggestions');
        if (alive) setSuggestions(Array.isArray(sugg) ? sugg : []);
      } catch {
        // Suggestions are purely additive — the section stands without them.
      }
      if (alive) setLoading(false);
    })();
    return () => {
      alive = false;
    };
  }, [isPro]);

  const create = useCallback(
    async (text: string, topic: string | null) => {
      const trimmed = text.trim();
      if (!trimmed || creating) return;
      setCreating(true);
      setCreatingTopic(topic);
      setActionError(null);
      try {
        await cmd('create_standing_query', { queryText: trimmed });
        setQueryText('');
        await refresh();
      } catch (e) {
        if (isSignalGateError(e)) setGated(true);
        else setActionError(describeCreateError(e, t));
      } finally {
        setCreating(false);
        setCreatingTopic(null);
      }
    },
    [creating, refresh, t],
  );

  const remove = useCallback(
    async (id: number) => {
      setActionError(null);
      try {
        await cmd('delete_standing_query', { id });
        setQueries((prev) => prev.filter((q) => q.id !== id));
      } catch (e) {
        if (isSignalGateError(e)) setGated(true);
        else setActionError(translateError(e));
      }
    },
    [],
  );

  const atCap = queries.length >= MAX_ACTIVE_QUERIES;
  const existing = new Set(queries.map((q) => q.query_text.toLowerCase()));
  const openSuggestions = suggestions.filter((s) => !existing.has(s.topic.toLowerCase()));

  // Locked state — mirrors the app's standard Signal gate (lock badge + CTA).
  if (!isPro || gated) {
    return (
      <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
        <div className="mb-3">
          <span className="block text-sm font-medium text-text-primary">{t('settings.standingQueries.title')}</span>
          <span className="block text-xs text-text-muted mt-0.5 leading-relaxed">{t('settings.standingQueries.desc')}</span>
        </div>
        <div className="flex flex-col items-center justify-center text-center gap-2 py-4">
          <div className="w-9 h-9 rounded-full bg-accent-gold/10 border border-accent-gold/20 flex items-center justify-center">
            <span className="text-accent-gold text-sm" aria-hidden="true">&#x1F512;</span>
          </div>
          <p className="text-sm font-medium text-text-primary">{t('settings.standingQueries.locked.title')}</p>
          <p className="text-xs text-text-muted max-w-sm">{t('settings.standingQueries.locked.subtitle')}</p>
          <div className="mt-1">
            <SignalUpgradeCTA compact source="settings-standing-queries" />
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <div className="mb-3 flex items-start justify-between gap-3">
        <div className="min-w-0">
          <span className="block text-sm font-medium text-text-primary">{t('settings.standingQueries.title')}</span>
          <span className="block text-xs text-text-muted mt-0.5 leading-relaxed">{t('settings.standingQueries.desc')}</span>
        </div>
        {queries.length > 0 && (
          <span className="shrink-0 text-[11px] text-text-muted tabular-nums mt-0.5">
            {t('settings.standingQueries.count', { count: queries.length, max: MAX_ACTIVE_QUERIES })}
          </span>
        )}
      </div>

      {loading ? (
        <p className="text-xs text-text-muted py-2">{t('settings.standingQueries.loading')}</p>
      ) : loadError ? (
        <p className="text-xs text-error py-2" role="alert">{t('settings.standingQueries.loadError')}</p>
      ) : (
        <>
          {/* Create form */}
          {atCap ? (
            <p className="text-xs text-text-muted mb-3">{t('settings.standingQueries.maxReached')}</p>
          ) : (
            <form
              className="flex gap-2 mb-3"
              onSubmit={(e) => {
                e.preventDefault();
                void create(queryText, null);
              }}
            >
              <input
                type="text"
                value={queryText}
                onChange={(e) => setQueryText(e.target.value)}
                aria-label={t('settings.standingQueries.inputLabel')}
                placeholder={t('settings.standingQueries.inputPlaceholder')}
                disabled={creating}
                className="flex-1 bg-bg-secondary border border-border rounded-lg px-3 py-2 text-xs text-text-primary placeholder-text-muted focus:border-accent-gold focus:outline-none disabled:opacity-60"
              />
              <button
                type="submit"
                disabled={creating || queryText.trim() === ''}
                className="px-3 py-2 text-xs font-medium text-text-secondary bg-bg-secondary border border-border rounded-lg hover:text-text-primary hover:border-accent-gold transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {creating && creatingTopic === null
                  ? t('settings.standingQueries.creating')
                  : t('settings.standingQueries.create')}
              </button>
            </form>
          )}

          {actionError && (
            <p className="text-xs text-error mb-3" role="alert">{actionError}</p>
          )}

          {/* One-click suggestion chips from engagement patterns */}
          {openSuggestions.length > 0 && !atCap && (
            <div className="mb-3">
              <p className="text-[10px] uppercase tracking-wider text-text-muted mb-1.5">
                {t('settings.standingQueries.suggestionsTitle')}
              </p>
              <div className="flex flex-wrap gap-1.5">
                {openSuggestions.map((s) => (
                  <button
                    key={s.topic}
                    type="button"
                    onClick={() => { void create(s.topic, s.topic); }}
                    disabled={creating}
                    title={s.reason}
                    aria-label={t('settings.standingQueries.suggestionAria', { topic: s.topic })}
                    className="px-2.5 py-1 text-[11px] rounded-full border border-border bg-bg-secondary text-text-secondary hover:text-text-primary hover:border-accent-gold transition-colors disabled:opacity-50"
                  >
                    {creating && creatingTopic === s.topic
                      ? t('settings.standingQueries.creating')
                      : `+ ${s.topic}`}
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Query list / inviting first-query state */}
          {queries.length === 0 ? (
            <div className="py-3 text-center">
              <p className="text-xs font-medium text-text-primary">{t('settings.standingQueries.emptyTitle')}</p>
              <p className="text-xs text-text-muted mt-1 max-w-md mx-auto">{t('settings.standingQueries.emptyDesc')}</p>
            </div>
          ) : (
            <ul className="space-y-1.5">
              {queries.map((q) => (
                <StandingQueryRow key={q.id} query={q} onDelete={(id) => { void remove(id); }} />
              ))}
            </ul>
          )}
        </>
      )}
    </div>
  );
});
