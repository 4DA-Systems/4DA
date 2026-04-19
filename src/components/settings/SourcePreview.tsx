// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import type { ParsedInput } from '../../utils/source-input-parser';

interface SourcePreviewProps {
  /** The parsed input — drives the preview mode. */
  parsed: ParsedInput;
  /** Triggered when the user confirms. */
  onConfirm: () => void;
  /** Optional cancel handler (clear the input). */
  onCancel?: () => void;
}

interface FeedTestState {
  loading: boolean;
  feedTitle: string | null;
  format: string;
  itemCount: number;
  items: Array<{ title: string; published_at: string | null }>;
  errors: string[];
}

/**
 * Shows a live preview of what 4DA will pull from a source before the user
 * commits to adding it. Runs a lightweight test fetch on mount, surfaces
 * errors as friendly messages, and only enables "Add" when the fetch succeeds.
 */
export function SourcePreview({ parsed, onConfirm, onCancel }: SourcePreviewProps) {
  const { t } = useTranslation();
  const [testState, setTestState] = useState<FeedTestState>({
    loading: parsed.kind === 'rss',
    feedTitle: null,
    format: '',
    itemCount: 0,
    items: [],
    errors: [],
  });

  useEffect(() => {
    // Only RSS gets a live test — YouTube/Twitter/Languages are validated on save.
    if (parsed.kind !== 'rss') {
      setTestState({
        loading: false,
        feedTitle: null,
        format: '',
        itemCount: 0,
        items: [],
        errors: [],
      });
      return;
    }

    let cancelled = false;
    setTestState(s => ({ ...s, loading: true, errors: [] }));

    void cmd('toolkit_test_feed', { url: parsed.value })
      .then(result => {
        if (cancelled) return;
        setTestState({
          loading: false,
          feedTitle: result.feed_title ?? null,
          format: result.format,
          itemCount: result.item_count,
          items: result.items.slice(0, 3).map(i => ({
            title: i.title,
            published_at: i.published_at,
          })),
          errors: result.errors,
        });
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        const message = err instanceof Error ? err.message : String(err);
        setTestState(s => ({
          ...s,
          loading: false,
          errors: [message],
        }));
      });

    return () => {
      cancelled = true;
    };
  }, [parsed.kind, parsed.value]);

  const canAdd =
    parsed.kind !== 'rss'
      ? parsed.kind !== 'unknown'
      : !testState.loading && testState.errors.length === 0 && testState.itemCount > 0;

  return (
    <div className="mt-2 rounded-lg border border-border bg-bg-secondary p-3">
      {/* Header — detected kind + the normalized value */}
      <div className="flex items-start justify-between gap-3 mb-2">
        <div className="min-w-0 flex-1">
          <p className="text-xs text-text-muted">{parsed.detected}</p>
          <p className="font-mono text-xs text-text-secondary mt-0.5 truncate" title={parsed.value}>
            {parsed.value}
          </p>
          {parsed.warnings.map((w, i) => (
            <p key={i} className="text-[11px] text-amber-400 mt-1">{w}</p>
          ))}
        </div>
      </div>

      {/* RSS-specific preview */}
      {parsed.kind === 'rss' && (
        <div className="mb-3">
          {testState.loading && (
            <p className="text-xs text-text-muted animate-pulse">
              {t('sources.preview.testing', 'Testing feed…')}
            </p>
          )}
          {!testState.loading && testState.errors.length > 0 && (
            <div className="rounded border border-red-500/30 bg-red-500/10 p-2 text-xs text-red-400">
              <p className="font-medium mb-0.5">{t('sources.preview.errorHeading', 'Feed not reachable')}</p>
              {testState.errors.map((e, i) => <p key={i}>{e}</p>)}
            </div>
          )}
          {!testState.loading && testState.errors.length === 0 && testState.itemCount > 0 && (
            <div className="rounded border border-green-500/20 bg-green-500/[0.05] p-2">
              <div className="flex items-center gap-2 text-xs text-green-400 mb-2">
                <span>{'\u2713'}</span>
                <span className="font-medium">
                  {testState.feedTitle ?? t('sources.preview.feedFound', 'Feed found')}
                </span>
                <span className="ms-auto text-text-muted">
                  {t('sources.preview.itemCount', '{{count}} items', { count: testState.itemCount })}
                </span>
              </div>
              <ul className="space-y-0.5 text-[11px]">
                {testState.items.map((item, i) => (
                  <li key={i} className="text-text-secondary truncate" title={item.title}>
                    &#x2022; {item.title}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}

      {/* Non-RSS types: just a confirm summary */}
      {parsed.kind !== 'rss' && parsed.kind !== 'unknown' && (
        <p className="text-xs text-text-secondary mb-3">
          {t('sources.preview.confirmNonRss', 'Ready to add. 4DA will fetch content on the next analysis run.')}
        </p>
      )}

      {parsed.kind === 'unknown' && (
        <p className="text-xs text-amber-400 mb-3">
          {parsed.detected}
        </p>
      )}

      {/* Actions */}
      <div className="flex gap-2">
        <button
          type="button"
          onClick={onConfirm}
          disabled={!canAdd}
          className="flex-1 px-3 py-1.5 text-xs rounded border border-green-500/30 bg-green-500/10 text-green-400 hover:bg-green-500/20 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {t('sources.preview.confirmAdd', 'Add source')}
        </button>
        {onCancel && (
          <button
            type="button"
            onClick={onCancel}
            className="px-3 py-1.5 text-xs rounded border border-border text-text-secondary hover:text-white hover:border-white/20 transition-colors"
          >
            {t('action.cancel', 'Cancel')}
          </button>
        )}
      </div>
    </div>
  );
}
