// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useMemo } from 'react';

import type { EvidenceFeed } from '../../src-tauri/bindings/bindings/EvidenceFeed';
import type { EvidenceItem } from '../../src-tauri/bindings/bindings/EvidenceItem';
import {
  type DepRow, type DepStatus, URGENCY_ORDER,
  depFromItem, signalMatchesDep,
} from '../components/blindspots/types';

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
    const missed = items.filter(it => it.id.startsWith('bs_missed_') || it.id.startsWith('llm-bs-'));
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
      const key = dep.toLowerCase();
      if (!depMap.has(key)) {
        depMap.set(key, {
          name: dep, status: 'falling_behind', urgency: signal.urgency,
          gap: null, signals: [], projects: [],
        });
      }
      depMap.get(key)!.signals.push(signal);
      matchedSignalIds.add(signal.id);
    }

    for (const row of depMap.values()) {
      if (row.gap && (row.gap.urgency === 'critical' || row.gap.urgency === 'high')) {
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

    const statusOrder: Record<DepStatus, number> = { blind_spot: 0, falling_behind: 1, well_covered: 2 };
    const rows = Array.from(depMap.values()).sort((a, b) =>
      statusOrder[a.status] - statusOrder[b.status]
      || URGENCY_ORDER[a.urgency] - URGENCY_ORDER[b.urgency]
    );

    const unmatched = missed.filter(m => !matchedSignalIds.has(m.id));
    return { depRows: rows, unmatchedSignals: unmatched, recommendations: recs };
  }, [report, dismissed]);
}
