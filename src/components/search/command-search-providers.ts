// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Search providers — the extensible registry behind the command search.
 *
 * Three tiers:
 *  - `goto`  (sync)  — jump to a view or settings. Instant, never fails.
 *  - `action` (sync) — trigger an app action. Instant, never fails.
 *  - `intelligence` (async) — search what 4DA has read via the existing
 *    `natural_language_query` backend. Additive; degrades to empty on any
 *    error (free-tier gate, embeddings down, backend stall) so the
 *    deterministic tiers always carry the experience.
 *
 * Adding a new searchable surface = appending a `SearchProvider`. The renderer
 * never changes.
 */

import { cmd } from '../../lib/commands';
import type { NLQResult } from '../../lib/commands';
import { openExternalUrl } from '../../lib/open-url';
import type { ActiveView } from '../../store/types';
import {
  type CommandResult,
  type SearchProvider,
  type ProviderContext,
  fuzzyScore,
} from './command-search-types';

export interface ProviderDeps {
  t: (key: string, fallback?: string) => string;
  setActiveView: (view: ActiveView) => void;
  onAnalyze: () => void;
  onOpenSettings: () => void;
  /** Ask the Signal view to scroll to + expand a specific item after navigating to it. */
  setSearchFocusItemId: (id: number | null) => void;
  /** True if the item is present in the current Signal feed (so the deep-link can land on it). */
  isItemInFeed: (id: number) => boolean;
}

/** Minimum query length before the async intelligence backend is queried. */
const INTELLIGENCE_MIN_CHARS = 2;
/** Hard cap on intelligence rows surfaced in the palette (doctrine: keep lists small). */
const INTELLIGENCE_MAX_ROWS = 6;

// ----------------------------------------------------------------------------
// Navigation (deterministic)
// ----------------------------------------------------------------------------

interface NavEntry {
  view: ActiveView;
  labelKey: string;
  keywords: string;
}

const NAV_ENTRIES: readonly NavEntry[] = [
  { view: 'briefing', labelKey: 'nav.briefing.label', keywords: 'brief daily morning summary today' },
  { view: 'preemption', labelKey: 'nav.preemption.label', keywords: 'alerts preempt risk ahead warning' },
  { view: 'blindspots', labelKey: 'nav.blindspots.label', keywords: 'coverage gaps missing blind spots' },
  { view: 'results', labelKey: 'nav.signal.label', keywords: 'signal feed results items relevant' },
  { view: 'playbook', labelKey: 'nav.playbook', keywords: 'streets guide lessons playbook learn' },
];

function navProvider(deps: ProviderDeps): SearchProvider {
  return {
    id: 'nav',
    group: 'goto',
    kind: 'sync',
    query({ query }: ProviderContext): CommandResult[] {
      const goPrefix = deps.t('cmdk.goToPrefix', 'Go to');
      const results: CommandResult[] = [];

      for (const entry of NAV_ENTRIES) {
        const label = deps.t(entry.labelKey);
        const score = Math.max(fuzzyScore(query, label), fuzzyScore(query, entry.keywords));
        if (score < 0) continue;
        results.push({
          id: `goto-${entry.view}`,
          group: 'goto',
          title: label,
          subtitle: goPrefix,
          score,
          run: () => deps.setActiveView(entry.view),
        });
      }

      // Settings is reachable from the launcher too.
      const settingsLabel = deps.t('header.settings', 'Settings');
      const settingsScore = Math.max(fuzzyScore(query, settingsLabel), fuzzyScore(query, 'settings preferences config options'));
      if (settingsScore >= 0) {
        results.push({
          id: 'goto-settings',
          group: 'goto',
          title: settingsLabel,
          subtitle: deps.t('cmdk.goToPrefix', 'Go to'),
          score: settingsScore,
          run: deps.onOpenSettings,
        });
      }

      return results;
    },
  };
}

// ----------------------------------------------------------------------------
// Actions (deterministic)
// ----------------------------------------------------------------------------

function actionProvider(deps: ProviderDeps): SearchProvider {
  interface ActionEntry {
    id: string;
    title: string;
    keywords: string;
    run: () => void;
  }

  const entries: ActionEntry[] = [
    {
      id: 'action-analyze',
      title: deps.t('action.runAnalysis', 'Run analysis'),
      keywords: 'analyze scan refresh fetch run now update',
      run: deps.onAnalyze,
    },
    {
      id: 'action-settings',
      title: deps.t('header.settings', 'Settings'),
      keywords: 'settings preferences configuration api key provider',
      run: deps.onOpenSettings,
    },
  ];

  return {
    id: 'actions',
    group: 'action',
    kind: 'sync',
    query({ query }: ProviderContext): CommandResult[] {
      return entries
        .map(e => ({ e, score: Math.max(fuzzyScore(query, e.title), fuzzyScore(query, e.keywords)) }))
        .filter(({ score }) => score >= 0)
        .map(({ e, score }) => ({
          id: e.id,
          group: 'action' as const,
          title: e.title,
          subtitle: deps.t('cmdk.actionPrefix', 'Action'),
          score,
          run: e.run,
        }));
    },
  };
}

// ----------------------------------------------------------------------------
// Intelligence (async, best-effort)
// ----------------------------------------------------------------------------

function firstLine(text: string, max = 80): string {
  const line = text.split('\n')[0]?.trim() ?? '';
  return line.length > max ? `${line.slice(0, max - 1)}…` : line;
}

function intelligenceProvider(deps: ProviderDeps): SearchProvider {
  return {
    id: 'intelligence',
    group: 'intelligence',
    kind: 'async',
    async query({ query, signal }: ProviderContext): Promise<CommandResult[]> {
      if (query.length < INTELLIGENCE_MIN_CHARS) return [];

      let res: NLQResult;
      try {
        res = await cmd('natural_language_query', { queryText: query });
      } catch {
        // Free-tier gate, embeddings unavailable, backend stall — the
        // intelligence tier is purely additive, so swallow and yield nothing.
        return [];
      }
      if (signal.aborted) return [];

      const items = (res.items ?? []).slice(0, INTELLIGENCE_MAX_ROWS);
      const results: CommandResult[] = items.map(item => ({
        id: `nlq-${item.id}`,
        group: 'intelligence' as const,
        title: item.file_name?.trim() || firstLine(item.preview) || deps.t('cmdk.untitledItem', 'Untitled item'),
        subtitle: item.match_reason?.trim() || item.source_type,
        score: item.relevance,
        badge: item.relevance.toFixed(2),
        run: () => {
          // Search spans the whole indexed corpus, but the Signal feed only holds the
          // current analysis results. Pick the action that actually works for THIS item:
          if (deps.isItemInFeed(item.id)) {
            // In the feed → deep-link: navigate and scroll to + expand it in context.
            deps.setSearchFocusItemId(item.id);
            deps.setActiveView('results');
          } else if (item.file_path) {
            // Off-feed (the common case) → open the source, like clicking a feed/brief item.
            openExternalUrl(item.file_path);
          } else {
            // No source URL and not in the feed → best-effort navigate to Signal.
            deps.setActiveView('results');
          }
        },
      }));

      // Ghost upsell row — free tier sees how much more Signal unlocks.
      const ghost = res.ghost_preview;
      if (ghost && ghost.hidden_results > 0) {
        results.push({
          id: 'nlq-ghost',
          group: 'intelligence',
          title: deps.t('cmdk.ghostUnlock', '{{count}} more matches — unlock with Signal').replace(
            '{{count}}',
            String(ghost.hidden_results),
          ),
          score: -1, // always sorts last within the group
          run: deps.onOpenSettings,
        });
      }

      return results;
    },
  };
}

/** Build the ordered provider registry for the current app context. */
export function buildProviders(deps: ProviderDeps): SearchProvider[] {
  return [navProvider(deps), actionProvider(deps), intelligenceProvider(deps)];
}
