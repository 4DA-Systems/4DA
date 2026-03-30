// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState, useEffect, useRef, memo } from 'react';
import { cmd } from '../../lib/commands';
import type { AweDecisionSummary } from '../../types/awe';

// ============================================================================
// Types
// ============================================================================

interface Props {
  topic: string;
}

// ============================================================================
// Helpers
// ============================================================================

/** Simple keyword overlap check between topic and decision statement */
function hasKeywordOverlap(topic: string, statement: string): boolean {
  const topicWords = new Set(
    topic.toLowerCase().split(/\s+/).filter(w => w.length > 3),
  );
  const stmtWords = statement.toLowerCase().split(/\s+/);
  return stmtWords.some(w => topicWords.has(w));
}

// ============================================================================
// Main Component
// ============================================================================

/**
 * ResultDecisionBacklink — inline backlink on expanded result items.
 *
 * Loads AWE decision history once (cached in ref), searches for keyword overlap
 * with the given topic, and renders a small gold chip when a match is found.
 * Returns null when no match exists.
 */
export const ResultDecisionBacklink = memo(function ResultDecisionBacklink({ topic }: Props) {
  const [match, setMatch] = useState<AweDecisionSummary | null>(null);
  const cacheRef = useRef<AweDecisionSummary[] | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        // Use cached data if available
        let decisions = cacheRef.current;
        if (!decisions) {
          const raw = await cmd('get_awe_decision_history', {
            domain: 'software-engineering',
            limit: 50,
          });
          decisions = JSON.parse(raw) as AweDecisionSummary[];
          cacheRef.current = decisions;
        }

        if (cancelled) return;

        // Find first decision with keyword overlap
        const found = decisions.find(d => hasKeywordOverlap(topic, d.statement));
        setMatch(found ?? null);
      } catch {
        // AWE unavailable — silent
      }
    }

    void load();
    return () => { cancelled = true; };
  }, [topic]);

  if (!match) return null;

  const outcomeLabel = match.outcome ?? 'pending';

  return (
    <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-accent-gold/10 border border-accent-gold/20">
      <span className="text-accent-gold text-[9px]">{'\u25C6'}</span>
      <span className="text-[10px] text-accent-gold/80 max-w-[200px] truncate">
        {match.statement}
      </span>
      <span className="text-[9px] text-text-muted italic">{outcomeLabel}</span>
    </span>
  );
});
