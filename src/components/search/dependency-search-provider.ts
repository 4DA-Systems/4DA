// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * "Your dependencies" — a typed package name finds what 4DA knows about it.
 *
 * Audit 2026-10-11: "sharp vulnerability" returned four news articles and not
 * the version-confirmed sharp advisory at the top of the user's own Preemption
 * worklist; "next.js security advisory" returned none of the six Next.js
 * advisories. The intelligence tier ranks what 4DA has READ; the dependency
 * facts are the product (AD-054), so they get their own group above it.
 *
 * Reads the Preemption feed already in the store (the materializer output,
 * with the user's dismissals and tier applied), so it is sync and instant. The
 * search field loads that feed when it opens if no view has loaded it yet.
 */

import { formatProjectNames } from '../../utils/project-path';

import type { EvidenceFeed } from '../../../src-tauri/bindings/bindings/EvidenceFeed';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import type { Urgency } from '../../../src-tauri/bindings/bindings/Urgency';
import type { PreemptionSubView } from '../../store/types';
import type { CommandResult, SearchProvider } from './command-search-types';

/** Doctrine: keep lists small. */
const DEPENDENCY_MAX_ROWS = 4;
/** A single partial word shorter than this matches nothing (no "re" → react, request…). */
const PREFIX_MIN_CHARS = 3;
/**
 * A prefix hit scores in a band below every exact hit, whatever its urgency:
 * "vite" must list vite before a high-urgency vitest (seen on the live feed).
 */
const PREFIX_BAND = 0.6;

const URGENCY_SCORE: Record<Urgency, number> = { critical: 1, high: 0.9, medium: 0.8, watch: 0.7 };

export interface DependencyProviderDeps {
  t: (key: string, fallback?: string) => string;
  openPreemption: (sub: PreemptionSubView) => void;
  /** Live read at query time; null until a feed has loaded (or on a paywall/error). */
  getPreemptionFeed: () => EvidenceFeed | null;
}

/** `serde_with` and `serde-with` name the same crate. */
const norm = (s: string) => s.toLowerCase().replace(/_/g, '-');

/**
 * The package names a query can mean, one set per typed word:
 * `next.js` / `nextjs` → `next`, `sharp@0.33` → `sharp`, trailing punctuation dropped.
 */
export function queryTokens(query: string): string[][] {
  const out: string[][] = [];
  for (const raw of query.split(/[\s,;]+/)) {
    const word = norm(raw)
      .replace(/(?<=.)@[^@/]*$/, '')
      .replace(/^[^a-z0-9@]+|[^a-z0-9]+$/g, '');
    if (word.length < 2) continue;
    const forms = new Set([word]);
    if (word.endsWith('.js')) forms.add(word.slice(0, -3));
    else if (word.endsWith('js') && word.length > 4) forms.add(word.slice(0, -2));
    out.push([...forms].filter(f => f.length >= 2));
  }
  return out;
}

/** `@ai-sdk/provider-utils` is also typed as `provider-utils`. */
function depKeys(name: string): string[] {
  const n = norm(name);
  const slash = n.indexOf('/');
  return n.startsWith('@') && slash > 0 ? [n, n.slice(slash + 1)] : [n];
}

export interface DependencyMatch {
  item: EvidenceItem;
  score: number;
}

/** Alerts whose affected packages the query names, best first. */
export function matchDependencyAlerts(items: readonly EvidenceItem[], query: string): DependencyMatch[] {
  const words = queryTokens(query);
  if (words.length === 0) return [];
  const allForms = new Set(words.flat());
  const partial = words.length === 1 ? (words[0] ?? []).filter(f => f.length >= PREFIX_MIN_CHARS) : [];

  const matches: DependencyMatch[] = [];
  for (const item of items) {
    const keys = item.affected_deps.flatMap(depKeys);
    let fit = 0;
    if (keys.some(k => allForms.has(k))) fit = 1;
    else if (keys.some(k => partial.some(p => k.startsWith(p)))) fit = PREFIX_BAND;
    if (fit > 0) matches.push({ item, score: URGENCY_SCORE[item.urgency] * fit });
  }
  return matches.sort((a, b) => b.score - a.score || a.item.title.localeCompare(b.item.title));
}

/** The worklist's own labels ("4da/src-tauri", never "src-tauri, src-tauri"), at most three. */
function projectsLabel(paths: string[]): string | undefined {
  const names = formatProjectNames(paths);
  if (names.length === 0) return undefined;
  return names.length <= 3 ? names.join(', ') : `${names.slice(0, 3).join(', ')} +${names.length - 3}`;
}

export function dependencyProvider(deps: DependencyProviderDeps): SearchProvider {
  return {
    id: 'dependencies',
    group: 'dependency',
    kind: 'sync',
    query({ query }): CommandResult[] {
      const feed = deps.getPreemptionFeed();
      if (!feed || query.trim().length === 0) return [];
      return matchDependencyAlerts(feed.items, query)
        .slice(0, DEPENDENCY_MAX_ROWS)
        .map(({ item, score }) => ({
          id: `dep-${item.id}`,
          group: 'dependency' as const,
          title: item.title,
          subtitle: projectsLabel(item.affected_projects),
          score,
          badge: deps.t(`preemption.urgency.${item.urgency}`, item.urgency),
          run: () => deps.openPreemption('worklist'),
        }));
    },
  };
}
