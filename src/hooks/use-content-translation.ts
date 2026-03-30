/**
 * useContentTranslation — translates dynamic content (feed items, briefings)
 * into the user's preferred language.
 *
 * Shows original text immediately, swaps to translated text when ready.
 * Translations are cached in SQLite so repeated views are instant.
 *
 * @example
 * const { translate, getTranslated } = useContentTranslation();
 *
 * // In a list component:
 * useEffect(() => {
 *   const items = results.map(r => ({ id: String(r.id), text: r.title }));
 *   translate(items);
 * }, [results]);
 *
 * // In render:
 * <span>{getTranslated(String(item.id)) ?? item.title}</span>
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import type { ContentTranslationRequest } from '../lib/commands';

interface TranslationMap {
  [id: string]: {
    original: string;
    translated: string;
    from_cache: boolean;
    provider: string;
  };
}

interface UseContentTranslationReturn {
  /** Submit items for translation. Deduplicates and batches automatically. */
  translate: (items: ContentTranslationRequest[]) => void;
  /** Get translated text for an item by ID. Returns null if not yet translated. */
  getTranslated: (id: string) => string | null;
  /** Whether a translation batch is currently in progress. */
  isTranslating: boolean;
  /** Whether content translation is enabled (non-English language + LLM configured). */
  isEnabled: boolean;
  /** Map of all completed translations. */
  translations: TranslationMap;
}

export function useContentTranslation(): UseContentTranslationReturn {
  const { i18n } = useTranslation();
  const [translations, setTranslations] = useState<TranslationMap>({});
  const [isTranslating, setIsTranslating] = useState(false);
  const [isEnabled, setIsEnabled] = useState(false);
  const pendingRef = useRef<Set<string>>(new Set());
  const batchTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const batchQueueRef = useRef<ContentTranslationRequest[]>([]);

  // Check if content translation is enabled
  useEffect(() => {
    const lang = i18n.language;
    if (lang === 'en') {
      setIsEnabled(false);
      return;
    }

    cmd('get_content_translation_settings')
      .then((settings) => setIsEnabled(settings.enabled))
      .catch(() => setIsEnabled(false));
  }, [i18n.language]);

  // Flush the batch queue to the backend
  const flushBatch = useCallback(async () => {
    const items = batchQueueRef.current;
    batchQueueRef.current = [];

    if (items.length === 0) return;

    setIsTranslating(true);
    try {
      const results = await cmd('translate_content_batch', { items });

      setTranslations((prev) => {
        const next = { ...prev };
        for (const result of results) {
          next[result.id] = {
            original: result.original,
            translated: result.translated,
            from_cache: result.from_cache,
            provider: result.provider,
          };
          pendingRef.current.delete(result.id);
        }
        return next;
      });
    } catch {
      // Translation failed — items stay with original text
      for (const item of items) {
        pendingRef.current.delete(item.id);
      }
    } finally {
      setIsTranslating(false);
    }
  }, []);

  // Submit items for translation with debounced batching
  const translate = useCallback(
    (items: ContentTranslationRequest[]) => {
      if (!isEnabled) return;

      // Filter out already translated and pending items
      const newItems = items.filter(
        (item) => !translations[item.id] && !pendingRef.current.has(item.id),
      );

      if (newItems.length === 0) return;

      // Mark as pending
      for (const item of newItems) {
        pendingRef.current.add(item.id);
      }

      // Add to batch queue
      batchQueueRef.current.push(...newItems);

      // Debounce: wait 100ms for more items before flushing
      if (batchTimeoutRef.current) {
        clearTimeout(batchTimeoutRef.current);
      }
      batchTimeoutRef.current = setTimeout(flushBatch, 100);
    },
    [isEnabled, translations, flushBatch],
  );

  // Get a translated string by ID
  const getTranslated = useCallback(
    (id: string): string | null => {
      const entry = translations[id];
      if (!entry) return null;
      // Only return translated text if it differs from original
      // (provider "none" means no translation was possible)
      if (entry.provider === 'none') return null;
      return entry.translated;
    },
    [translations],
  );

  // Clear translations when language changes
  useEffect(() => {
    setTranslations({});
    pendingRef.current.clear();
    batchQueueRef.current = [];
  }, [i18n.language]);

  return { translate, getTranslated, isTranslating, isEnabled, translations };
}
