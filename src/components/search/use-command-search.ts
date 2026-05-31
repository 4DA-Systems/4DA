// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Orchestration for the command search: runs sync providers instantly, debounces
 * async providers, merges + groups results, and guards against out-of-order
 * async responses (a fast typist's prior query resolving after a newer one).
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { buildProviders, type ProviderDeps } from './command-search-providers';
import { GROUP_ORDER, type CommandGroup, type CommandResult } from './command-search-types';
import { frecencyBoost } from '../../lib/frecency';

const DEBOUNCE_MS = 180;
const ASYNC_MIN_CHARS = 2;
const ASYNC_CACHE_MAX = 24;

function orderResults(all: CommandResult[]): CommandResult[] {
  const byGroup = new Map<CommandGroup, CommandResult[]>();
  for (const r of all) {
    const arr = byGroup.get(r.group) ?? [];
    arr.push(r);
    byGroup.set(r.group, arr);
  }
  const ordered: CommandResult[] = [];
  for (const g of GROUP_ORDER) {
    const items = byGroup.get(g);
    if (items) ordered.push(...items.sort((a, b) => b.score - a.score));
  }
  return ordered;
}

export interface UseCommandSearch {
  query: string;
  setQuery: (q: string) => void;
  results: CommandResult[];
  loading: boolean;
  activeId: string | null;
  setActiveId: (id: string | null) => void;
  moveActive: (delta: number) => void;
  runActive: () => void;
  reset: () => void;
}

export function useCommandSearch(deps: ProviderDeps): UseCommandSearch {
  const { t, setActiveView, onAnalyze, onOpenSettings, setSearchFocusItemId, isItemInFeed } = deps;
  const providers = useMemo(
    () => buildProviders({ t, setActiveView, onAnalyze, onOpenSettings, setSearchFocusItemId, isItemInFeed }),
    [t, setActiveView, onAnalyze, onOpenSettings, setSearchFocusItemId, isItemInFeed],
  );

  const [query, setQueryState] = useState('');
  const [syncResults, setSyncResults] = useState<CommandResult[]>([]);
  const [asyncResults, setAsyncResults] = useState<CommandResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [activeId, setActiveId] = useState<string | null>(null);

  const reqIdRef = useRef(0);
  const abortRef = useRef<AbortController | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // LRU cache of async (intelligence) results, keyed by trimmed query, so
  // backspacing or re-running a recent query is instant and skips the backend.
  const asyncCacheRef = useRef<Map<string, CommandResult[]>>(new Map());

  const runProviders = useCallback((rawQuery: string) => {
    const trimmed = rawQuery.trim();
    const reqId = ++reqIdRef.current;

    // Stale-response guard: abort any in-flight async run before starting a new one.
    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;
    if (debounceRef.current) clearTimeout(debounceRef.current);

    // 1. Sync providers resolve instantly — they carry the experience.
    const sync: CommandResult[] = [];
    for (const p of providers) {
      if (p.kind !== 'sync') continue;
      try {
        const out = p.query({ query: trimmed, signal: controller.signal });
        if (Array.isArray(out)) sync.push(...out);
      } catch {
        /* a single provider must never break the palette */
      }
    }
    setSyncResults(sync);

    // 2. Async providers are debounced + abortable, and only fire once the
    //    query is substantial enough to be worth a backend round-trip.
    const asyncProviders = providers.filter(p => p.kind === 'async');
    const asyncEligible = trimmed.length >= ASYNC_MIN_CHARS && asyncProviders.length > 0;
    if (!asyncEligible) {
      setAsyncResults([]);
      setLoading(false);
      return;
    }
    // LRU cache hit — serve instantly, no debounce, no backend round-trip.
    const cache = asyncCacheRef.current;
    const cached = cache.get(trimmed);
    if (cached) {
      cache.delete(trimmed);
      cache.set(trimmed, cached); // bump recency
      setAsyncResults(cached);
      setLoading(false);
      return;
    }

    setLoading(true);
    debounceRef.current = setTimeout(() => {
      void Promise.allSettled(
        asyncProviders.map(p => Promise.resolve(p.query({ query: trimmed, signal: controller.signal }))),
      ).then(settled => {
        // Drop the response if a newer keystroke superseded it.
        if (reqId !== reqIdRef.current || controller.signal.aborted) return;
        const out: CommandResult[] = [];
        for (const s of settled) if (s.status === 'fulfilled') out.push(...s.value);
        // Store in the LRU, evicting the oldest entry past capacity.
        cache.set(trimmed, out);
        if (cache.size > ASYNC_CACHE_MAX) {
          const oldest = cache.keys().next().value;
          if (oldest !== undefined) cache.delete(oldest);
        }
        setAsyncResults(out);
        setLoading(false);
      });
    }, DEBOUNCE_MS);
  }, [providers]);

  const setQuery = useCallback((q: string) => {
    setQueryState(q);
    runProviders(q);
  }, [runProviders]);

  useEffect(() => () => {
    abortRef.current?.abort();
    if (debounceRef.current) clearTimeout(debounceRef.current);
  }, []);

  const results = useMemo(() => {
    // Apply the frecency boost before ordering so frequently/recently chosen
    // results float up within their group — the palette learns your habits.
    const merged = [...syncResults, ...asyncResults].map(r => ({
      ...r,
      score: r.score + frecencyBoost(r.id),
    }));
    return orderResults(merged);
  }, [syncResults, asyncResults]);

  // Maintain a valid active selection, defaulting to the first row.
  useEffect(() => {
    setActiveId(prev => {
      if (results.length === 0) return null;
      return prev && results.some(r => r.id === prev) ? prev : (results[0]?.id ?? null);
    });
  }, [results]);

  const moveActive = useCallback((delta: number) => {
    setActiveId(prev => {
      if (results.length === 0) return null;
      const idx = Math.max(0, results.findIndex(r => r.id === prev));
      const next = (idx + delta + results.length) % results.length;
      return results[next]?.id ?? null;
    });
  }, [results]);

  const runActive = useCallback(() => {
    const item = results.find(r => r.id === activeId) ?? results[0];
    item?.run();
  }, [results, activeId]);

  const reset = useCallback(() => {
    setQueryState('');
    setSyncResults([]);
    setAsyncResults([]);
    setLoading(false);
    setActiveId(null);
    abortRef.current?.abort();
    if (debounceRef.current) clearTimeout(debounceRef.current);
  }, []);

  return { query, setQuery, results, loading, activeId, setActiveId, moveActive, runActive, reset };
}
