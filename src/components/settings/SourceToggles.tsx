// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * What 4DA reads (AD-054 decision 3).
 *
 * Stack sources (package registries and advisories) are what the dependency
 * engine needs, so they are listed as always on. Interests (news, social and
 * editorial reading) are opt-in: off on a new install, and off after the
 * schema-126 migration on an existing one, until the user turns them on here.
 */
import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd, type SourceSetting } from '../../lib/commands';
import { translateError } from '../../utils/error-messages';

interface SourceTogglesProps {
  onStatusChange: (status: string) => void;
}

type LoadState =
  | { kind: 'loading' }
  | { kind: 'error'; message: string }
  | { kind: 'ready'; rows: SourceSetting[] };

export function SourceToggles({ onStatusChange }: SourceTogglesProps) {
  const { t } = useTranslation();
  const [state, setState] = useState<LoadState>({ kind: 'loading' });
  const [saving, setSaving] = useState<string | null>(null);

  const load = useCallback(async () => {
    setState({ kind: 'loading' });
    try {
      const rows = await cmd('get_source_settings');
      setState({ kind: 'ready', rows });
    } catch (e) {
      setState({ kind: 'error', message: translateError(e) });
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const toggle = useCallback(
    async (row: SourceSetting) => {
      const next = !row.enabled;
      setSaving(row.source_type);
      try {
        const saved = await cmd('set_source_enabled', { sourceType: row.source_type, enabled: next });
        setState((prev) =>
          prev.kind === 'ready'
            ? { kind: 'ready', rows: prev.rows.map((r) => (r.source_type === saved.source_type ? saved : r)) }
            : prev,
        );
        onStatusChange(
          t(next ? 'sources.toggles.turnedOn' : 'sources.toggles.turnedOff', { name: row.name }),
        );
      } catch (e) {
        onStatusChange(t('sources.toggles.saveError', { name: row.name, error: translateError(e) }));
      } finally {
        setSaving(null);
      }
    },
    [onStatusChange, t],
  );

  return (
    <section
      className="bg-bg-tertiary rounded-lg p-4 border border-border mb-4"
      aria-labelledby="source-toggles-title"
    >
      <h3 id="source-toggles-title" className="text-text-primary font-medium">
        {t('sources.toggles.title')}
      </h3>
      <p className="text-text-muted text-sm mt-0.5">{t('sources.toggles.subtitle')}</p>

      {state.kind === 'loading' && (
        <p className="text-sm text-text-muted mt-3" role="status">
          {t('sources.toggles.loading')}
        </p>
      )}

      {state.kind === 'error' && (
        <div className="mt-3 flex items-center gap-3" role="alert">
          <p className="text-sm text-error flex-1">
            {t('sources.toggles.error', { error: state.message })}
          </p>
          <button
            onClick={() => { void load(); }}
            className="px-3 py-1.5 text-xs bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-text-primary transition-colors"
          >
            {t('sources.toggles.retry')}
          </button>
        </div>
      )}

      {state.kind === 'ready' && state.rows.length === 0 && (
        <p className="text-sm text-text-muted mt-3">{t('sources.toggles.empty')}</p>
      )}

      {state.kind === 'ready' && state.rows.length > 0 && (
        <div className="mt-3 space-y-4">
          <SourceGroup
            heading={t('sources.toggles.stackHeading')}
            hint={t('sources.toggles.stackHint')}
            rows={state.rows.filter((r) => r.class === 'stack')}
            renderControl={() => (
              <span className="text-xs text-text-muted">{t('sources.toggles.alwaysOn')}</span>
            )}
          />
          <SourceGroup
            heading={t('sources.toggles.interestsHeading')}
            hint={t('sources.toggles.interestsHint')}
            rows={state.rows.filter((r) => r.class === 'interest')}
            renderControl={(row) => (
              <button
                role="switch"
                aria-checked={row.enabled}
                aria-label={t('sources.toggles.toggleLabel', { name: row.name })}
                disabled={saving !== null}
                onClick={() => { void toggle(row); }}
                className={`relative w-9 h-5 rounded-full transition-colors shrink-0 ${row.enabled ? 'bg-success' : 'bg-border'} disabled:opacity-60`}
              >
                <span
                  className={`absolute top-0.5 start-0.5 w-4 h-4 rounded-full bg-bg-primary transition-transform ${row.enabled ? 'translate-x-4 rtl:-translate-x-4' : 'translate-x-0'}`}
                />
              </button>
            )}
          />
        </div>
      )}
    </section>
  );
}

interface SourceGroupProps {
  heading: string;
  hint: string;
  rows: SourceSetting[];
  renderControl: (row: SourceSetting) => ReactNode;
}

function SourceGroup({ heading, hint, rows, renderControl }: SourceGroupProps) {
  if (rows.length === 0) return null;
  return (
    <div>
      <h4 className="text-xs font-medium text-text-secondary uppercase tracking-wide">{heading}</h4>
      <p className="text-xs text-text-muted mt-0.5 mb-2">{hint}</p>
      <ul className="divide-y divide-border border border-border rounded-lg bg-bg-secondary">
        {rows.map((row) => (
          <li key={row.source_type} className="flex items-center justify-between gap-3 px-3 py-2">
            <span className="text-sm text-text-primary truncate">{row.name}</span>
            {renderControl(row)}
          </li>
        ))}
      </ul>
    </div>
  );
}
