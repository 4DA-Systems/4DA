// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, type KeyboardEvent, type ReactNode } from 'react';

export interface SubViewTab<Id extends string> {
  id: Id;
  label: string;
  /** Optional trailing marker (e.g. the Signal tier label on a gated view). */
  marker?: ReactNode;
}

interface SubViewTabsProps<Id extends string> {
  /** Accessible name of the tablist. */
  label: string;
  /** Prefix for element ids: tabs are `${idPrefix}-tab-${id}`, panels `${idPrefix}-panel-${id}`. */
  idPrefix: string;
  tabs: ReadonlyArray<SubViewTab<Id>>;
  selected: Id;
  onSelect: (id: Id) => void;
}

/** Element id of the panel a sub-view tab controls. */
export function subViewPanelId(idPrefix: string, id: string): string {
  return `${idPrefix}-panel-${id}`;
}

/** Element id of a sub-view tab (the panel's `aria-labelledby`). */
export function subViewTabId(idPrefix: string, id: string): string {
  return `${idPrefix}-tab-${id}`;
}

/**
 * A compact segmented sub-navigation — the visual pattern of Signal's
 * List | Themes | Graph toggle — with WAI-ARIA tabs semantics and a roving
 * tabindex (the same keyboard model as the main ViewTabBar): only the selected
 * tab is in the Tab order; arrows / Home / End move focus and activate.
 */
function SubViewTabsInner<Id extends string>({ label, idPrefix, tabs, selected, onSelect }: SubViewTabsProps<Id>) {
  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>, current: number) => {
    const list = e.currentTarget.closest('[role="tablist"]');
    if (!list) return;
    const rtl = window.getComputedStyle(list).direction === 'rtl';
    const forward = rtl ? 'ArrowLeft' : 'ArrowRight';
    const backward = rtl ? 'ArrowRight' : 'ArrowLeft';
    let next: number;
    if (e.key === forward) next = (current + 1) % tabs.length;
    else if (e.key === backward) next = (current - 1 + tabs.length) % tabs.length;
    else if (e.key === 'Home') next = 0;
    else if (e.key === 'End') next = tabs.length - 1;
    else return;
    e.preventDefault();
    const tab = tabs[next]!;
    list.querySelector<HTMLButtonElement>(`#${subViewTabId(idPrefix, tab.id)}`)?.focus();
    onSelect(tab.id);
  };

  const selectedIndex = tabs.findIndex((tab) => tab.id === selected);
  const focusableIndex = selectedIndex >= 0 ? selectedIndex : 0;

  return (
    <div
      role="tablist"
      aria-label={label}
      aria-orientation="horizontal"
      className="inline-flex flex-wrap rounded-lg border border-border bg-bg-secondary p-0.5"
    >
      {tabs.map((tab, index) => {
        const isSelected = tab.id === selected;
        return (
          <button
            key={tab.id}
            type="button"
            role="tab"
            id={subViewTabId(idPrefix, tab.id)}
            aria-selected={isSelected}
            aria-controls={subViewPanelId(idPrefix, tab.id)}
            tabIndex={index === focusableIndex ? 0 : -1}
            onClick={() => onSelect(tab.id)}
            onKeyDown={(e) => onKeyDown(e, index)}
            className={`inline-flex items-center gap-1.5 px-3 py-1 text-xs font-medium rounded-md transition-colors ${
              isSelected ? 'bg-bg-tertiary text-text-primary' : 'text-text-muted hover:text-text-secondary'
            }`}
          >
            <span>{tab.label}</span>
            {tab.marker}
          </button>
        );
      })}
    </div>
  );
}

export const SubViewTabs = memo(SubViewTabsInner) as typeof SubViewTabsInner;
