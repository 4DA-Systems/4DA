// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useMemo } from 'react';

import type { EvidenceFeed } from '../../src-tauri/bindings/bindings/EvidenceFeed';
import type { EvidenceItem } from '../../src-tauri/bindings/bindings/EvidenceItem';
import {
  type DepRow, type DepStatus, URGENCY_ORDER,
  barePackageName, depFromItem, signalMatchesDep,
} from '../components/blindspots/types';
import { normalizeUrlForDedup } from '../utils/normalize-url';

/**
 * One story, one slot: collapse missed signals that point at the same URL.
 * The backend dedups by title similarity, but the same story fetched via two
 * sources (mastodon toot + HN submission of the same safedep.io post) carries
 * different titles over one URL — live audit 2026-08-31, the arrayref story
 * held 6 of 24 Emerging slots, the same URL twice among them. The list
 * arrives backend-ranked, so the first (highest-ranked) copy survives.
 */
function dedupSignalsByUrl(signals: EvidenceItem[]): EvidenceItem[] {
  const seen = new Set<string>();
  return signals.filter(signal => {
    const key = normalizeUrlForDedup(signal.evidence[0]?.url);
    if (!key) return true;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export interface BlindSpotsData {
  depRows: DepRow[];
  unmatchedSignals: EvidenceItem[];
  recommendations: EvidenceItem[];
}

/**
 * Transforms raw EvidenceFeed items into categorized dependency rows,
 * unmatched signals, and recommendations — filtering out dismissed items.
 */
export function useBlindSpotsData(
  report: EvidenceFeed | null,
  dismissed: Set<string>,
): BlindSpotsData {
  return useMemo(() => {
    const items = (report?.items ?? []).filter(it => !dismissed.has(it.id));

    const gaps = items.filter(it => it.id.startsWith('bs_uncov_') || it.id.startsWith('bs_stale_'));
    const missed = dedupSignalsByUrl(
      items.filter(it => it.id.startsWith('bs_missed_') || it.id.startsWith('llm-bs-')),
    );
    const recs = items.filter(it => it.id.startsWith('bs_rec_'));

    const depMap = new Map<string, DepRow>();

    for (const gap of gaps) {
      const dep = depFromItem(gap);
      if (!dep) continue;
      const key = dep.toLowerCase();
      if (!depMap.has(key)) {
        depMap.set(key, {
          name: dep, status: 'blind_spot', urgency: gap.urgency,
          gap, signals: [], projects: gap.affected_projects,
        });
      }
    }

    const matchedSignalIds = new Set<string>();

    for (const signal of missed) {
      for (const [, row] of depMap) {
        if (signalMatchesDep(signal, row.name)) {
          row.signals.push(signal);
          matchedSignalIds.add(signal.id);
          break;
        }
      }
    }

    for (const signal of missed) {
      if (matchedSignalIds.has(signal.id)) continue;
      const dep = depFromItem(signal);
      if (!dep) continue;
      // Key by BARE package name so a signal carrying "react" lands on the
      // existing "react (npm)" row instead of minting a second, bare row
      // beside it (live audit 2026-08-31: "react" with "3 signals", no
      // ecosystem, no project, rendered next to "react (npm)"). The
      // signalMatchesDep pass above already absorbs most of these; this keeps
      // the row-creation lane from resurrecting the shadow for any that slip
      // through (e.g. a dep name that never appears in the signal title).
      const key = barePackageName(dep).toLowerCase();
      let row = depMap.get(key) ?? null;
      if (!row) {
        for (const existing of depMap.values()) {
          if (barePackageName(existing.name).toLowerCase() === key) {
            row = existing;
            break;
          }
        }
      }
      if (!row) {
        row = {
          name: dep, status: 'falling_behind', urgency: signal.urgency,
          gap: null, signals: [], projects: [],
        };
        depMap.set(key, row);
      }
      row.signals.push(signal);
      matchedSignalIds.add(signal.id);
    }

    for (const row of depMap.values()) {
      // Zero-coverage first (2026-08-31 audit): a gap the backend classified
      // as "zero available signals" has NO unreviewed activity — it must not
      // inflate the "Drifting / unreviewed activity" tier (20+ of its rows
      // literally said "Sources were checked but found no results"). If the
      // frontend lane DID match visible missed signals to it, there is
      // activity after all, so the normal tiers apply.
      if (row.gap?.lens_hints.no_coverage === true && row.signals.length === 0) {
        row.status = 'no_coverage';
      } else if (row.gap && (row.gap.urgency === 'critical' || row.gap.urgency === 'high')) {
        row.status = 'blind_spot';
      } else if (row.gap || row.signals.length >= 3) {
        row.status = row.signals.length > 0 ? 'blind_spot' : 'falling_behind';
      } else if (row.signals.length > 0) {
        row.status = 'falling_behind';
      } else {
        row.status = 'well_covered';
      }
      row.signals.sort((a, b) => URGENCY_ORDER[a.urgency] - URGENCY_ORDER[b.urgency]);
    }

    const statusOrder: Record<DepStatus, number> = { blind_spot: 0, falling_behind: 1, no_coverage: 2, well_covered: 3 };
    const rows = Array.from(depMap.values()).sort((a, b) =>
      statusOrder[a.status] - statusOrder[b.status]
      || URGENCY_ORDER[a.urgency] - URGENCY_ORDER[b.urgency]
    );

    const unmatched = missed.filter(m => !matchedSignalIds.has(m.id));
    return { depRows: rows, unmatchedSignals: unmatched, recommendations: recs };
  }, [report, dismissed]);
}
