// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useCallback, useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useAppStore } from '../../store';
import { isModK, modShortcutLabel } from '../../lib/platform';
import { recordPick } from '../../lib/frecency';
import { useCommandSearch } from './use-command-search';
import { CommandSearchResults } from './CommandSearchResults';
import { type CommandResult } from './command-search-types';

interface CommandSearchProps {
  onAnalyze: () => void;
  onOpenSettings: () => void;
}

/** Below this viewport width the field collapses to an icon that opens a centered overlay. */
const COMPACT_BREAKPOINT = 720;

const PANEL_CLASS = 'max-h-[60vh] overflow-y-auto rounded-lg border border-border bg-bg-secondary shadow-2xl py-1.5';

export const CommandSearch = memo(function CommandSearch({ onAnalyze, onOpenSettings }: CommandSearchProps) {
  const { t } = useTranslation();
  const setActiveView = useAppStore(s => s.setActiveView);
  const setSearchFocusItemId = useAppStore(s => s.setSearchFocusItemId);

  // Adapt i18next's TFunction to the providers' simple (key, fallback?) => string contract.
  const translate = useCallback(
    (key: string, fallback?: string): string =>
      fallback === undefined ? t(key) : t(key, fallback),
    [t],
  );

  // Live check (read at click time, not render time) of whether an item is in the current feed.
  const isItemInFeed = useCallback(
    (id: number) => useAppStore.getState().appState.relevanceResults.some(r => r.id === id),
    [],
  );

  const search = useCommandSearch({ t: translate, setActiveView, onAnalyze, onOpenSettings, setSearchFocusItemId, isItemInFeed });
  const { query, setQuery, results, loading, activeId, setActiveId, moveActive, reset } = search;

  const [open, setOpen] = useState(false);
  const [compact, setCompact] = useState(() => typeof window !== 'undefined' && window.innerWidth < COMPACT_BREAKPOINT);
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listboxId = useId();

  const close = useCallback(() => {
    setOpen(false);
    inputRef.current?.blur();
  }, []);

  const openAndFocus = useCallback(() => {
    setOpen(true);
    const el = inputRef.current; // may be null in compact mode until the overlay mounts
    if (el) {
      el.focus();
      el.select();
    }
  }, []);

  // Track viewport width for the compact collapse.
  useEffect(() => {
    const onResize = () => setCompact(window.innerWidth < COMPACT_BREAKPOINT);
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  // In compact mode the input only exists once the overlay opens, so focus it on mount.
  useEffect(() => {
    if (open && compact) inputRef.current?.focus();
  }, [open, compact]);

  // Global Cmd/Ctrl+K — platform-correct (⌘ on macOS, Ctrl on Windows/Linux).
  // The `/` focus shortcut is handled by the app's keyboard layer via the
  // [data-search-input] hook target, which fires this input's onFocus.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (isModK(e, 'k')) {
        e.preventDefault();
        openAndFocus();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [openAndFocus]);

  // Close when clicking outside the field/panel (containerRef wraps the field in
  // inline mode and the inner panel in overlay mode, so a backdrop click closes).
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) close();
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [open, close]);

  const handleSelect = useCallback((item: CommandResult) => {
    recordPick(item.id); // frecency: learn what the user chooses
    item.run();
    reset();
    close();
  }, [reset, close]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setOpen(true);
        moveActive(1);
        break;
      case 'ArrowUp':
        e.preventDefault();
        moveActive(-1);
        break;
      case 'Enter': {
        const item = results.find(r => r.id === activeId);
        if (item) {
          e.preventDefault();
          handleSelect(item);
        }
        break;
      }
      case 'Escape':
        if (open) {
          e.preventDefault();
          e.stopPropagation();
          close();
        }
        break;
    }
  }, [results, activeId, moveActive, handleSelect, open, close]);

  const showDropdown = open && (results.length > 0 || query.trim().length > 0 || loading);
  const hasQuery = query.trim().length > 0;

  // Compact + closed: a single icon button that opens the overlay.
  if (compact && !open) {
    return (
      <button
        type="button"
        aria-label={t('cmdk.ariaLabel', 'Search 4DA')}
        onClick={openAndFocus}
        className="w-8 h-8 flex items-center justify-center rounded-md bg-bg-secondary text-text-secondary border border-border hover:bg-bg-tertiary hover:border-orange-500/30 transition-all"
      >
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
          <circle cx="6" cy="6" r="4.3" stroke="currentColor" strokeWidth="1.4" />
          <path d="M9.4 9.4L12.5 12.5" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
        </svg>
      </button>
    );
  }

  const fieldBox = (
    <div
      className={`flex items-center gap-2 h-[34px] px-2.5 rounded-lg border bg-bg-secondary transition-colors ${
        open ? 'border-orange-500/40' : 'border-border hover:border-border/80'
      }`}
    >
      <svg width="14" height="14" viewBox="0 0 14 14" fill="none" className="flex-shrink-0 opacity-60" aria-hidden="true">
        <circle cx="6" cy="6" r="4.3" stroke="currentColor" strokeWidth="1.4" />
        <path d="M9.4 9.4L12.5 12.5" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
      </svg>
      <input
        ref={inputRef}
        data-search-input
        type="text"
        role="combobox"
        aria-expanded={showDropdown}
        aria-controls={listboxId}
        aria-activedescendant={activeId ? `cmd-opt-${activeId}` : undefined}
        aria-autocomplete="list"
        aria-label={t('cmdk.ariaLabel', 'Search 4DA')}
        spellCheck={false}
        autoComplete="off"
        value={query}
        placeholder={t('cmdk.placeholder', 'Search everything 4DA has read…')}
        onChange={e => { setQuery(e.target.value); setOpen(true); }}
        onFocus={() => setOpen(true)}
        onKeyDown={handleKeyDown}
        className="flex-1 min-w-0 bg-transparent text-sm text-text-primary placeholder:text-text-muted outline-none"
      />
      {hasQuery ? (
        <button
          type="button"
          aria-label={t('cmdk.clear', 'Clear')}
          onClick={() => { reset(); inputRef.current?.focus(); }}
          className="flex-shrink-0 w-4 h-4 flex items-center justify-center rounded text-text-muted hover:text-text-secondary"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true"><path d="M2 2l6 6M8 2l-6 6" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" /></svg>
        </button>
      ) : (
        <kbd className="flex-shrink-0 text-[10px] text-text-muted bg-bg-tertiary border border-border rounded px-1.5 py-px font-mono">
          {modShortcutLabel('K')}
        </kbd>
      )}
    </div>
  );

  const resultsList = showDropdown && (
    <CommandSearchResults
      listboxId={listboxId}
      ariaLabel={t('cmdk.ariaLabel', 'Search 4DA')}
      className={compact ? `mt-2 ${PANEL_CLASS}` : `absolute left-0 right-0 top-full mt-2 z-50 ${PANEL_CLASS}`}
      results={results}
      activeId={activeId}
      loading={loading}
      hasQuery={hasQuery}
      onHover={setActiveId}
      onSelect={handleSelect}
    />
  );

  // Compact + open: centered overlay palette.
  if (compact) {
    return (
      <div className="fixed inset-0 z-50 flex justify-center items-start pt-16 px-4 bg-black/40" role="presentation">
        <div ref={containerRef} className="w-full max-w-[520px]">
          {fieldBox}
          {resultsList}
        </div>
      </div>
    );
  }

  // Inline (default): field in the top bar with an absolute dropdown.
  return (
    <div ref={containerRef} className="relative w-full max-w-[440px]">
      {fieldBox}
      {resultsList}
    </div>
  );
});
