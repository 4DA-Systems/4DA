// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * ContentTranslationProvider — app-level provider for real-time content translation.
 *
 * Wraps the app and provides translated text for feed items via React context.
 * Components use `useTranslatedContent(id, originalText)` to get translated text.
 *
 * The provider watches the store for relevance results and automatically
 * translates new items when the user's language is not English.
 */

import { createContext, memo, useCallback, useContext, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import type { ContentTranslationRequest } from '../lib/commands';

// ============================================================================
// Context
// ============================================================================

interface ContentTranslationContextValue {
  /** Get translated text for an item. Returns original if no translation available. */
  getTranslated: (id: string, original: string) => string;
  /** Whether content translation is active (non-English language + LLM configured). */
  isActive: boolean;
  /** Whether a translation batch is currently in progress. */
  isTranslating: boolean;
  /** Request translation for specific items. */
  requestTranslation: (items: ContentTranslationRequest[]) => void;
  /** Why translation is not active (empty string if active). */
  inactiveReason: string;
}

const ContentTranslationContext = createContext<ContentTranslationContextValue>({
  getTranslated: (_id, original) => original,
  isActive: false,
  isTranslating: false,
  requestTranslation: () => {},
  inactiveReason: '',
});

// ============================================================================
// Provider
// ============================================================================

export const ContentTranslationProvider = memo(function ContentTranslationProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const { i18n } = useTranslation();
  const [translations, setTranslations] = useState<Record<string, string>>({});
  const [isActive, setIsActive] = useState(false);
  const [isTranslating, setIsTranslating] = useState(false);
  const [inactiveReason, setInactiveReason] = useState('');
  const pendingRef = useRef<Set<string>>(new Set());
  const batchQueueRef = useRef<ContentTranslationRequest[]>([]);
  const batchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Detect if content translation should be active
  useEffect(() => {
    if (i18n.language === 'en') {
      setIsActive(false);
      setInactiveReason('');
      return;
    }

    // Sync language to backend settings
    cmd('set_locale', { country: '', language: i18n.language, currency: '' }).catch(() => {});

    cmd('get_content_translation_settings')
      .then((settings) => {
        setIsActive(settings.enabled);
        if (!settings.enabled) {
          setInactiveReason(
            settings.provider === 'disabled'
              ? 'no_llm'
              : '',
          );
        } else {
          setInactiveReason('');
        }
      })
      .catch(() => {
        setIsActive(false);
        setInactiveReason('error');
      });
  }, [i18n.language]);

  // Clear translations on language change
  useEffect(() => {
    setTranslations({});
    pendingRef.current.clear();
    batchQueueRef.current = [];
  }, [i18n.language]);

  // Flush batch queue
  const flushBatch = useCallback(async () => {
    const items = batchQueueRef.current;
    batchQueueRef.current = [];
    if (items.length === 0) return;

    setIsTranslating(true);
    try {
      const results = await cmd('translate_content_batch', { items });
      setTranslations((prev) => {
        const next = { ...prev };
        for (const r of results) {
          if (r.provider !== 'none') {
            next[r.id] = r.translated;
          }
          pendingRef.current.delete(r.id);
        }
        return next;
      });
    } catch {
      for (const item of items) {
        pendingRef.current.delete(item.id);
      }
    } finally {
      setIsTranslating(false);
    }
  }, []);

  // Request translation for items (debounced batching)
  const requestTranslation = useCallback(
    (items: ContentTranslationRequest[]) => {
      if (!isActive) return;

      const newItems = items.filter(
        (item) => !translations[item.id] && !pendingRef.current.has(item.id),
      );
      if (newItems.length === 0) return;

      for (const item of newItems) {
        pendingRef.current.add(item.id);
      }
      batchQueueRef.current.push(...newItems);

      if (batchTimerRef.current) clearTimeout(batchTimerRef.current);
      batchTimerRef.current = setTimeout(flushBatch, 150);
    },
    [isActive, translations, flushBatch],
  );

  // Get translated text, falling back to original
  const getTranslated = useCallback(
    (id: string, original: string): string => {
      return translations[id] ?? original;
    },
    [translations],
  );

  return (
    <ContentTranslationContext.Provider
      value={{ getTranslated, isActive, isTranslating, requestTranslation, inactiveReason }}
    >
      {children}
    </ContentTranslationContext.Provider>
  );
});

// ============================================================================
// Consumer Hook
// ============================================================================

/**
 * useTranslatedContent — get translated text for a content item.
 *
 * @example
 * const { getTranslated, requestTranslation } = useTranslatedContent();
 *
 * // Request translation on mount/update:
 * useEffect(() => {
 *   requestTranslation(items.map(i => ({ id: String(i.id), text: i.title })));
 * }, [items]);
 *
 * // In render:
 * <span>{getTranslated(String(item.id), item.title)}</span>
 */
export function useTranslatedContent() {
  return useContext(ContentTranslationContext);
}
