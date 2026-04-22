// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback, useRef } from 'react';
import { cmd } from '../lib/commands';
import { translateError } from '../utils/error-messages';
import { useAppStore } from '../store';

export interface ItemSummaryState {
  summary: string | null;
  summaryLoading: boolean;
  summaryError: string | null;
  generateSummary: () => Promise<void>;
}

interface UseItemSummaryOptions {
  autoGenerate?: boolean;
}

export function useItemSummary(itemId: number, isExpanded: boolean, options?: UseItemSummaryOptions): ItemSummaryState {
  const [summary, setSummary] = useState<string | null>(null);
  const [summaryLoading, setSummaryLoading] = useState(false);
  const [summaryError, setSummaryError] = useState<string | null>(null);
  const autoGenTriggered = useRef(false);

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

  // Fetch cached summary when expanded
  useEffect(() => {
    if (!isExpanded) return;
    let cancelled = false;
    cmd('get_item_summary', { itemId })
      .then(result => {
        if (!cancelled) setSummary(result.summary);
      })
      .catch((e) => {
        console.warn('[4DA] Cached summary fetch failed:', e);
        if (!cancelled && options?.autoGenerate && !autoGenTriggered.current) {
          const apiKey = useAppStore.getState().settingsForm.apiKey;
          if (apiKey) {
            autoGenTriggered.current = true;
            void generateSummary();
          }
        }
      });
    return () => { cancelled = true; };
  }, [isExpanded, itemId, options?.autoGenerate, generateSummary]);

  // Reset auto-gen flag when item changes
  useEffect(() => {
    autoGenTriggered.current = false;
  }, [itemId]);

  return { summary, summaryLoading, summaryError, generateSummary };
}
