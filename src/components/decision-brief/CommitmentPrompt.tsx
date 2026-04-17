// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Commitment Prompt — Intelligence Reconciliation Phase 11 (2026-04-17).
 *
 * Shown when the user clicks Accept on a Decision Brief. Asks for a
 * one-sentence refutation condition and stores the commitment contract
 * via the backend.
 */

import { memo, useCallback, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

interface Props {
  decisionStatement: string;
  subject: string;
  onComplete: () => void;
  onCancel: () => void;
}

export const CommitmentPrompt = memo(function CommitmentPrompt({
  decisionStatement,
  subject,
  onComplete,
  onCancel,
}: Props) {
  const { t } = useTranslation();
  const [condition, setCondition] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);

  const handleSave = useCallback(async () => {
    const trimmed = condition.trim();
    if (trimmed.length === 0 || saving) return;
    setSaving(true);
    setError(null);
    try {
      await cmd('create_commitment_contract', {
        decisionStatement,
        refutationCondition: trimmed,
        subject,
      });
      onComplete();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }, [condition, saving, decisionStatement, subject, onComplete]);

  return (
    <div className="border-t border-border pt-4 mt-4 space-y-3">
      <h3 className="text-[10px] text-accent-gold uppercase tracking-wider font-medium">
        {t('commitment.title', 'Commitment Contract')}
      </h3>
      <p className="text-xs text-text-secondary leading-relaxed">
        {t(
          'commitment.question',
          'What would convince you that you chose wrong? 4DA will watch for it.',
        )}
      </p>
      <input
        ref={inputRef}
        type="text"
        value={condition}
        onChange={e => setCondition(e.target.value)}
        onKeyDown={e => {
          if (e.key === 'Enter') {
            e.preventDefault();
            void handleSave();
          }
        }}
        placeholder={t(
          'commitment.placeholder',
          'e.g. If build times go over 45 seconds',
        )}
        className="w-full bg-bg-primary border border-border rounded-md px-3 py-2 text-sm text-white placeholder:text-text-muted focus:outline-none focus:border-accent-gold/60"
        disabled={saving}
      />
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => void handleSave()}
          disabled={saving || condition.trim().length === 0}
          className="px-3 py-1.5 text-xs rounded-md bg-accent-gold/20 border border-accent-gold/40 text-accent-gold hover:bg-accent-gold/30 transition-colors disabled:opacity-50"
        >
          {saving
            ? t('commitment.saving', 'Saving...')
            : t('commitment.save', 'Set refutation watch')}
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="px-3 py-1.5 text-xs rounded-md border border-border text-text-muted hover:text-white transition-colors"
        >
          {t('commitment.skip', 'Skip')}
        </button>
      </div>
      {error !== null && (
        <p className="text-xs text-red-400" role="alert">
          {error}
        </p>
      )}
    </div>
  );
});
