// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Confession Box — Intelligence Reconciliation Phase 10 (2026-04-17).
 *
 * Global `⌘.` / `Ctrl+.` modal. Single text input. User types a
 * one-sentence decision; we call `run_awe_transmute` (which carries
 * the Phase-6 17-field context bridge); on return, render the
 * canonical DecisionBrief.
 *
 * This is the ONLY user-facing AWE entry point permitted by the
 * Intelligence Doctrine outside the lens surfaces.
 */

import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import { DecisionBrief, type DecisionBriefData } from './DecisionBrief';
import { parseBrief } from './brief-parser';
import { CommitmentPrompt } from './CommitmentPrompt';

interface Props {
  open: boolean;
  onClose: () => void;
}

export const ConfessionBox = memo(function ConfessionBox({ open, onClose }: Props) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [brief, setBrief] = useState<DecisionBriefData | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showCommitment, setShowCommitment] = useState(false);
  const [committed, setCommitted] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement | null>(null);

  // Reset state each time the modal opens.
  useEffect(() => {
    if (open) {
      setQuery('');
      setBrief(null);
      setLoading(false);
      setError(null);
      setShowCommitment(false);
      setCommitted(false);
      // Auto-focus the input so the user can type immediately.
      setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open]);

  // ESC closes the modal.
  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    }
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  const handleSubmit = useCallback(async () => {
    const trimmed = query.trim();
    if (trimmed.length === 0 || loading) return;
    setLoading(true);
    setError(null);
    try {
      const raw = await cmd('run_awe_transmute', {
        query: trimmed,
        mode: 'structured',
      });
      setBrief(parseBrief(raw, trimmed));
    } catch (e: unknown) {
      const msg = e instanceof Error
        ? e.message
        : typeof e === 'string'
          ? e
          : typeof e === 'object' && e !== null && 'message' in e
            ? String((e as Record<string, unknown>).message)
            : JSON.stringify(e);
      if (msg.includes('AWE') || msg.includes('awe') || msg.includes('os error') || msg.includes('not found')) {
        setError(
          'The Wisdom engine (AWE) is not available on this system. ' +
          'It requires a compatible binary — check docs/strategy for setup instructions.',
        );
      } else {
        setError(msg);
      }
    } finally {
      setLoading(false);
    }
  }, [query, loading]);

  // Cmd/Ctrl+Enter submits from inside the textarea.
  const onInputKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
        e.preventDefault();
        void handleSubmit();
      }
    },
    [handleSubmit],
  );

  if (!open) return null;

  // The backdrop is a button to satisfy a11y — click/Enter/Space close
  // the modal, and the inner content stops propagation.
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={t('confession.title', 'Wisdom')}
      className="fixed inset-0 z-50 flex items-start justify-center pt-[12vh] px-4"
    >
      <button
        type="button"
        aria-label={t('action.close', 'Close')}
        onClick={onClose}
        className="absolute inset-0 bg-black/70 backdrop-blur-sm cursor-default"
      />
      <div
        className="relative w-full max-w-2xl"
        onClick={e => e.stopPropagation()}
        onKeyDown={e => e.stopPropagation()}
        role="presentation"
      >
        {/* Header */}
        <div className="bg-bg-secondary border border-border rounded-t-lg px-5 py-3 flex items-center justify-between">
          <div className="flex items-baseline gap-2">
            <span className="text-accent-gold text-sm" aria-hidden="true">
              ◇
            </span>
            <h2 className="text-sm font-medium text-white">
              {t('confession.title', 'Wisdom')}
            </h2>
            <span className="text-[10px] text-text-muted">
              {t('confession.subtitle', 'describe a decision you are weighing')}
            </span>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={t('action.close', 'Close')}
            className="text-text-muted hover:text-white text-sm transition-colors"
          >
            ✕
          </button>
        </div>

        {/* Input */}
        {brief === null && (
          <div className="bg-bg-secondary border-x border-b border-border rounded-b-lg p-5 space-y-3">
            <textarea
              ref={inputRef}
              data-confession-input="true"
              value={query}
              onChange={e => setQuery(e.target.value)}
              onKeyDown={onInputKeyDown}
              placeholder={t(
                'confession.placeholder',
                "e.g. I'm torn between adopting Turborepo vs staying on pnpm workspaces…",
              )}
              rows={4}
              className="w-full resize-none bg-bg-primary border border-border rounded-md p-3 text-sm text-white placeholder:text-text-muted focus:outline-none focus:border-accent-gold/60"
              disabled={loading}
            />
            <div className="flex items-center justify-between">
              <span className="text-[10px] text-text-muted">
                {t('confession.hint', 'Ctrl+Enter to submit · Esc to close')}
              </span>
              <button
                type="button"
                onClick={() => void handleSubmit()}
                disabled={loading || query.trim().length === 0}
                className="px-4 py-1.5 text-xs rounded-md bg-accent-gold/20 border border-accent-gold/40 text-accent-gold hover:bg-accent-gold/30 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {loading
                  ? t('confession.thinking', 'Thinking…')
                  : t('confession.submit', 'Transmute')}
              </button>
            </div>
            {error !== null && (
              <p className="text-xs text-red-400" role="alert">
                {error}
              </p>
            )}
          </div>
        )}

        {/* Brief result */}
        {brief !== null && (
          <div className="bg-bg-secondary border-x border-b border-border rounded-b-lg p-5">
            <DecisionBrief
              data={brief}
              onAccept={committed ? undefined : () => {
                // Persist the decision so it survives modal close.
                // Non-blocking, best-effort — if persistence fails, the
                // commitment prompt still shows (the user shouldn't be
                // blocked by a backend write failure).
                void cmd('record_developer_decision', {
                  decisionType: 'wisdom_brief',
                  subject: query.trim().split(/\s+/).pop() ?? '',
                  decision: brief.decision,
                  rationale: brief.verdict,
                  alternativesRejected: [],
                  contextTags: brief.assumptions.slice(0, 3),
                  confidence: brief.confidence,
                }).catch(() => {});
                setShowCommitment(true);
              }}
              onDefer={() => {
                // Persist as deferred — decision not lost on close.
                void cmd('record_developer_decision', {
                  decisionType: 'wisdom_brief',
                  subject: query.trim().split(/\s+/).pop() ?? '',
                  decision: brief.decision,
                  rationale: 'Deferred for later consideration',
                  alternativesRejected: [],
                  contextTags: [],
                  confidence: brief.confidence,
                }).catch(() => {});
                setBrief(null);
                setQuery('');
                onClose();
              }}
              onReject={() => {
                setBrief(null);
                setQuery('');
                onClose();
              }}
            />

            {/* Commitment Contract prompt — shows after Accept */}
            {showCommitment && !committed && (
              <CommitmentPrompt
                decisionStatement={brief.decision}
                subject={query.trim().split(/\s+/).pop() ?? ''}
                onComplete={() => {
                  setCommitted(true);
                  setShowCommitment(false);
                }}
                onCancel={() => setShowCommitment(false)}
              />
            )}

            {committed && (
              <p className="mt-3 text-xs text-green-400">
                {t('commitment.confirmed', 'Refutation watch set. 4DA will surface it if the condition is met.')}
              </p>
            )}

            <div className="mt-4 flex justify-end">
              <button
                type="button"
                onClick={() => {
                  setBrief(null);
                  setQuery('');
                  setShowCommitment(false);
                  setCommitted(false);
                  setTimeout(() => inputRef.current?.focus(), 0);
                }}
                className="text-[11px] text-text-muted hover:text-white transition-colors"
              >
                {t('confession.again', 'Ask another')}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
});

export default ConfessionBox;
