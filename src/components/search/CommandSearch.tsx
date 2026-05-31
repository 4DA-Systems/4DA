// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useCallback, useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useAppStore } from '../../store';
import { isModK, modShortcutLabel } from '../../lib/platform';
import { useCommandSearch } from './use-command-search';
import { GROUP_ORDER, type CommandGroup, type CommandResult } from './command-search-types';

interface CommandSearchProps {
  onAnalyze: () => void;
  onOpenSettings: () => void;
}

const GROUP_LABEL_KEYS: Record<CommandGroup, string> = {
  goto: 'cmdk.groupGoto',
  action: 'cmdk.groupAction',
  intelligence: 'cmdk.groupIntelligence',
};

const GROUP_DEFAULT_LABEL: Record<CommandGroup, string> = {
  goto: 'Go to',
  action: 'Actions',
  intelligence: 'Intelligence',
};

function GroupGlyph({ group }: { group: CommandGroup }) {
  const common = { width: 14, height: 14, viewBox: '0 0 14 14', fill: 'none', 'aria-hidden': true } as const;
  if (group === 'goto') {
    return (
      <svg {...common}><path d="M3 7h8M7.5 3.5L11 7l-3.5 3.5" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" /></svg>
    );
  }
  if (group === 'action') {
    return (
      <svg {...common}><path d="M7.5 1.5L3 8h3.5L6 12.5 11 6H7L7.5 1.5Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" /></svg>
    );
  }
  return (
    <svg {...common}><path d="M7 1.8l1.5 3.7 3.7 1.5-3.7 1.5L7 12.2 5.5 8.5 1.8 7l3.7-1.5L7 1.8Z" stroke="currentColor" strokeWidth="1.1" strokeLinejoin="round" /></svg>
  );
}

export const CommandSearch = memo(function CommandSearch({ onAnalyze, onOpenSettings }: CommandSearchProps) {
  const { t } = useTranslation();
  const setActiveView = useAppStore(s => s.setActiveView);

  // Adapt i18next's TFunction to the providers' simple (key, fallback?) => string contract.
  const translate = useCallback(
    (key: string, fallback?: string): string =>
      fallback === undefined ? t(key) : t(key, fallback),
    [t],
  );

  const search = useCommandSearch({ t: translate, setActiveView, onAnalyze, onOpenSettings });
  const { query, setQuery, results, loading, activeId, setActiveId, moveActive, reset } = search;

  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listboxId = useId();

  const close = useCallback(() => {
    setOpen(false);
    inputRef.current?.blur();
  }, []);

  const openAndFocus = useCallback(() => {
    setOpen(true);
    const el = inputRef.current;
    if (el) {
      el.focus();
      el.select();
    }
  }, []);

  // Global Cmd/Ctrl+K — platform-correct (⌘ on macOS, Ctrl on Windows/Linux).
  // The `/` focus shortcut is already handled by the app's keyboard layer via
  // the [data-search-input] hook target, which fires this input's onFocus.
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

  // Close when clicking outside the field/dropdown.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) close();
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [open, close]);

  const handleSelect = useCallback((item: CommandResult) => {
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

  return (
    <div ref={containerRef} className="relative w-full max-w-[440px]">
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

      {showDropdown && (
        <div
          id={listboxId}
          role="listbox"
          aria-label={t('cmdk.ariaLabel', 'Search 4DA')}
          className="absolute left-0 right-0 top-full mt-2 z-50 max-h-[60vh] overflow-y-auto rounded-lg border border-border bg-bg-secondary shadow-2xl py-1.5"
        >
          {GROUP_ORDER.map(group => {
            const items = results.filter(r => r.group === group);
            if (items.length === 0) return null;
            return (
              <div key={group} className="px-1.5 pb-1">
                <div className="px-2 py-1 text-[10px] uppercase tracking-wider text-text-muted">
                  {t(GROUP_LABEL_KEYS[group], GROUP_DEFAULT_LABEL[group])}
                </div>
                {items.map(item => {
                  const isActive = item.id === activeId;
                  const isGhost = item.id === 'nlq-ghost';
                  return (
                    <button
                      key={item.id}
                      id={`cmd-opt-${item.id}`}
                      type="button"
                      role="option"
                      aria-selected={isActive}
                      onMouseEnter={() => setActiveId(item.id)}
                      onMouseDown={e => { e.preventDefault(); handleSelect(item); }}
                      className={`w-full flex items-center gap-2.5 px-2 py-1.5 rounded-md text-start transition-colors ${
                        isActive ? 'bg-bg-tertiary' : ''
                      }`}
                    >
                      <span className={`flex-shrink-0 ${isGhost ? 'text-accent-gold' : 'text-text-muted'}`}>
                        <GroupGlyph group={group} />
                      </span>
                      <span className="flex-1 min-w-0">
                        <span className={`block text-sm truncate ${isGhost ? 'text-accent-gold' : 'text-text-primary'}`}>{item.title}</span>
                        {item.subtitle && (
                          <span className="block text-xs text-text-muted truncate">{item.subtitle}</span>
                        )}
                      </span>
                      {item.badge && (
                        <span className="flex-shrink-0 text-[10px] font-mono text-text-muted bg-bg-tertiary border border-border rounded px-1.5 py-px">
                          {item.badge}
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>
            );
          })}

          {loading && (
            <div className="px-3.5 py-2 text-xs text-text-muted flex items-center gap-2">
              <span className="w-3 h-3 border-2 border-text-muted/40 border-t-text-muted rounded-full animate-spin" aria-hidden="true" />
              {t('cmdk.loading', 'Searching…')}
            </div>
          )}

          {!loading && results.length === 0 && hasQuery && (
            <div className="px-3.5 py-3 text-sm text-text-muted">
              {t('cmdk.empty', 'No matches')}
              <span className="block text-xs mt-0.5 text-text-muted/70">{t('cmdk.emptyHint', 'Try a topic, a view, or an action')}</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
});
