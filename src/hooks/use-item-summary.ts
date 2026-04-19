// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback } from 'react';
import { cmd } from '../lib/commands';
import { translateError } from '../utils/error-messages';

export interface ItemSummaryState {
  summary: string | null;
  summaryLoading: boolean;
  summaryError: string | null;
  generateSummary: () => Promise<void>;
}

export function useItemSummary(itemId: number, isExpanded: boolean): ItemSummaryState {
  const [summary, setSummary] = useState<string | null>(null);
  const [summaryLoading, setSummaryLoading] = useState(false);
  const [summaryError, setSummaryError] = useState<string | null>(null);

  // Fetch cached summary when expanded
  useEffect(() => {
    if (!isExpanded) return;
    let cancelled = false;
    cmd('get_item_summary', { itemId })
      .then(result => { if (!cancelled) setSummary(result.summary); })
      .catch((e) => {
        // Cache miss is expected — log but don't set error state
        // (user hasn't triggered anything, no need to surface this)
        console.warn('[4DA] Cached summary fetch failed:', e);
      });
    return () => { cancelled = true; };
  }, [isExpanded, itemId]);

  const generateSummary = useCallback(async () => {
    setSummaryLoading(true);
    setSummaryError(null);
    try {
      const result = await cmd('generate_item_summary', { itemId });
      setSummary(result.summary);
    } catch (e) {
      setSummaryError(translateError(e));
    } finally {
      setSummaryLoading(false);
    }
  }, [itemId]);

  return { summary, summaryLoading, summaryError, generateSummary };
}
