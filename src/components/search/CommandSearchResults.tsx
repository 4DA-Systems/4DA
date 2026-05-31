// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo } from 'react';
import { useTranslation } from 'react-i18next';

import { GROUP_ORDER, type CommandGroup, type CommandResult } from './command-search-types';

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

interface CommandSearchResultsProps {
  listboxId: string;
  ariaLabel: string;
  className: string;
  results: CommandResult[];
  activeId: string | null;
  loading: boolean;
  hasQuery: boolean;
  onHover: (id: string) => void;
  onSelect: (item: CommandResult) => void;
}

/**
 * Renders the grouped result listbox (sections + loading + empty states).
 * Positioning is the parent's concern — it passes `className`, so the same list
 * works as an absolute dropdown (inline field) or a static panel (overlay mode).
 */
export const CommandSearchResults = memo(function CommandSearchResults({
  listboxId,
  ariaLabel,
  className,
  results,
  activeId,
  loading,
  hasQuery,
  onHover,
  onSelect,
}: CommandSearchResultsProps) {
  const { t } = useTranslation();

  return (
    <div id={listboxId} role="listbox" aria-label={ariaLabel} className={className}>
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
                  onMouseEnter={() => onHover(item.id)}
                  onMouseDown={e => { e.preventDefault(); onSelect(item); }}
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
  );
});
