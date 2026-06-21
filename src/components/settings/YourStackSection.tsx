// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

interface ProjectRow {
  path: string;
  name: string;
  dependency_count: number;
  included: boolean;
}

/**
 * "Your Stack" — user-controlled project allowlist. Lists the locally-detected
 * projects that carry dependencies and lets the user toggle which ones count
 * toward relevance grounding ("Affects You"). Excluding a project (e.g. a test
 * fixture or scaffolding) drops its deps from scoring on the next analysis.
 * Optimistic toggle; reverts if the persist fails.
 */
export const YourStackSection = memo(function YourStackSection() {
  const { t } = useTranslation();
  const [projects, setProjects] = useState<ProjectRow[]>([]);
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

  const toggle = useCallback((path: string, nextIncluded: boolean) => {
    setProjects((prev) => prev.map((p) => (p.path === path ? { ...p, included: nextIncluded } : p))); // optimistic
    setSavingPath(path);
    void cmd('set_project_in_stack', { path, included: nextIncluded })
      .catch(() =>
        setProjects((prev) => prev.map((p) => (p.path === path ? { ...p, included: !nextIncluded } : p))),
      ) // revert on failure
      .finally(() => setSavingPath(null));
  }, []);

  const includedCount = projects.filter((p) => p.included).length;

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
            {t('settings.stack.summary', { included: includedCount, total: projects.length })}
          </p>
          <ul className="space-y-1.5 max-h-64 overflow-y-auto">
            {projects.map((p) => (
              <li key={p.path} className="flex items-center gap-3">
                <button
                  type="button"
                  role="switch"
                  aria-checked={p.included}
                  aria-label={p.name}
                  disabled={savingPath === p.path}
                  onClick={() => toggle(p.path, !p.included)}
                  className={`relative w-9 h-5 rounded-full transition-colors shrink-0 ${p.included ? 'bg-success' : 'bg-border'} disabled:opacity-60`}
                >
                  <span
                    className={`absolute top-0.5 start-0.5 w-4 h-4 rounded-full bg-white transition-transform ${p.included ? 'translate-x-4' : 'translate-x-0'}`}
                  />
                </button>
                <span className="min-w-0 flex-1">
                  <span className="block text-xs font-medium text-text-primary truncate" title={p.path}>
                    {p.name}
                  </span>
                  <span className="block text-[10px] text-text-muted truncate" title={p.path}>
                    {p.path}
                  </span>
                </span>
                <span className="text-[10px] text-text-muted shrink-0">
                  {t('settings.stack.deps', { count: p.dependency_count })}
                </span>
              </li>
            ))}
          </ul>
          <p className="text-[10px] text-text-muted mt-2">{t('settings.stack.applyNote')}</p>
        </>
      )}
    </div>
  );
});
