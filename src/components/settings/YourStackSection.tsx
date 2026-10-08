// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd, type StackProjectRow } from '../../lib/commands';
import { applyStackChoice, isInactiveProject } from './your-stack-choice';

/**
 * "Your Stack" — user-controlled project allowlist. Lists the locally-detected
 * projects that carry dependencies and lets the user toggle which ones count
 * toward relevance grounding ("Affects You"). Excluding a project (e.g. a test
 * fixture or scaffolding) drops its deps from scoring on the next analysis.
 *
 * The count and each row read the backend's verdict (`counts`), the SAME rule
 * grounding uses: a dormant (>90 days) or scratch (gitignored) project is
 * labelled and not counted unless the user forces it in (audit 2026-10-07).
 * Optimistic toggle; reverts if the persist fails.
 */
export const YourStackSection = memo(function YourStackSection() {
  const { t } = useTranslation();
  const [projects, setProjects] = useState<StackProjectRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [savingPath, setSavingPath] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    void cmd('list_projects_with_stack_status')
      .then((rows) => { if (alive) setProjects(rows); })
      .catch(() => { if (alive) setProjects([]); })
      .finally(() => { if (alive) setLoading(false); });
    return () => { alive = false; };
  }, []);

  const choose = useCallback((row: StackProjectRow, included: boolean, force: boolean) => {
    setProjects((prev) => prev.map((p) => (p.path === row.path ? applyStackChoice(p, included, force) : p))); // optimistic
    setSavingPath(row.path);
    void cmd('set_project_in_stack', { path: row.path, included, force })
      .catch(() => setProjects((prev) => prev.map((p) => (p.path === row.path ? row : p)))) // revert on failure
      .finally(() => setSavingPath(null));
  }, []);

  const countedCount = projects.filter((p) => p.counts).length;

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <div className="mb-3">
        <span className="block text-sm font-medium text-text-primary">{t('settings.stack.title')}</span>
        <span className="block text-xs text-text-muted mt-0.5 leading-relaxed">{t('settings.stack.desc')}</span>
      </div>
      {loading ? (
        <p className="text-xs text-text-muted py-2">{t('settings.stack.loading')}</p>
      ) : projects.length === 0 ? (
        <p className="text-xs text-text-muted py-2">{t('settings.stack.empty')}</p>
      ) : (
        <>
          <p className="text-[11px] text-text-muted mb-2">
            {t('settings.stack.summary', { included: countedCount, total: projects.length })}
          </p>
          <ul className="space-y-1.5 max-h-64 overflow-y-auto">
            {projects.map((p) => (
              <StackRow key={p.path} p={p} saving={savingPath === p.path} onChoose={choose} />
            ))}
          </ul>
          <p className="text-[10px] text-text-muted mt-2">{t('settings.stack.applyNote')}</p>
        </>
      )}
    </div>
  );
});

interface StackRowProps {
  p: StackProjectRow;
  saving: boolean;
  onChoose: (row: StackProjectRow, included: boolean, force: boolean) => void;
}

function StackRow({ p, saving, onChoose }: StackRowProps) {
  const { t } = useTranslation();
  const inactive = isInactiveProject(p);
  return (
    <li className="flex items-center gap-3">
      <button
        type="button"
        role="switch"
        aria-checked={p.included}
        aria-label={p.name}
        disabled={saving}
        onClick={() => onChoose(p, !p.included, false)}
        className={`relative w-9 h-5 rounded-full transition-colors shrink-0 ${p.counts ? 'bg-success' : 'bg-border'} disabled:opacity-60`}
      >
        <span
          className={`absolute top-0.5 start-0.5 w-4 h-4 rounded-full bg-white transition-transform ${p.included ? 'translate-x-4' : 'translate-x-0'}`}
        />
      </button>
      <span className="min-w-0 flex-1">
        <span className="flex items-center gap-1.5 min-w-0">
          <span className="text-xs font-medium text-text-primary truncate" title={p.path}>{p.name}</span>
          {p.dormant && p.dormant_days != null && (
            <span className="shrink-0 px-1 rounded text-[9px] bg-bg-secondary text-text-muted border border-border">
              {t('settings.stack.dormant', { days: p.dormant_days })}
            </span>
          )}
          {p.scratch && (
            <span className="shrink-0 px-1 rounded text-[9px] bg-bg-secondary text-text-muted border border-border">
              {t('settings.stack.scratch')}
            </span>
          )}
          {p.forced && p.included && (
            <span className="shrink-0 px-1 rounded text-[9px] bg-bg-secondary text-text-secondary border border-border">
              {t('settings.stack.forced')}
            </span>
          )}
        </span>
        <span className="block text-[10px] text-text-muted truncate" title={p.path}>{p.path}</span>
        {p.included && inactive && !p.forced && (
          <span className="block text-[10px] text-text-muted">{t('settings.stack.inactiveHint')}</span>
        )}
      </span>
      {p.included && inactive && (
        <button
          type="button"
          disabled={saving}
          onClick={() => onChoose(p, !p.forced, true)}
          className="text-[10px] shrink-0 px-1.5 py-0.5 rounded border border-border text-text-secondary hover:text-text-primary disabled:opacity-60"
        >
          {p.forced ? t('settings.stack.undoForce') : t('settings.stack.forceInclude')}
        </button>
      )}
      <span className="text-[10px] text-text-muted shrink-0">
        {t('settings.stack.deps', { count: p.dependency_count })}
      </span>
    </li>
  );
}
