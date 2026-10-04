// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import type { ReleaseChanges } from '../../../src-tauri/bindings/bindings/ReleaseChanges';

/**
 * "What changed" for a graded registry release, read from the changelog the
 * package ships in its own registry archive. Counts by kind, the first
 * breaking entries, and a link to the changelog itself. When there is no
 * changelog, or it cannot be read, the card says so — it never invents.
 */
export const ReleaseChangesCard = memo(function ReleaseChangesCard({
  changes,
  loading,
}: {
  changes: ReleaseChanges | null;
  loading: boolean;
}) {
  const { t } = useTranslation();
  if (loading && !changes) {
    return (
      <div className="mb-3 text-[11px] text-text-muted" data-testid="release-changes-loading">
        {t('results.releaseChanges.loading')}
      </div>
    );
  }
  if (!changes) return null;

  const range = { from: changes.from_version, to: changes.to_version };
  const found = changes.status === 'found';

  return (
    <div className="mb-3 p-2 bg-bg-primary/50 rounded border border-border/40" data-testid="release-changes">
      <div className="text-xs text-text-secondary font-medium mb-1">
        {t('results.releaseChanges.title', range)}
      </div>
      {found ? (
        <FoundBody changes={changes} />
      ) : (
        <p className="text-xs text-text-muted" data-testid="release-changes-empty">
          {emptyMessage(changes, t)}
        </p>
      )}
      {changes.source_url && changes.status !== 'unavailable' && (
        <a
          href={changes.source_url}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-block mt-1.5 text-[10px] text-text-muted hover:text-text-secondary underline"
        >
          {changes.file ? t('results.releaseChanges.readChangelog') : t('results.releaseChanges.viewPackage')}
        </a>
      )}
    </div>
  );
});

function FoundBody({ changes }: { changes: ReleaseChanges }) {
  const { t } = useTranslation();
  const extras = [
    changes.security > 0 ? t('results.releaseChanges.security', { count: changes.security }) : null,
    changes.deprecations > 0 ? t('results.releaseChanges.deprecations', { count: changes.deprecations }) : null,
  ].filter((x): x is string => x !== null);
  return (
    <>
      <div className="text-xs text-text-primary" data-testid="release-changes-counts">
        {t('results.releaseChanges.counts', {
          breaking: changes.breaking,
          features: changes.features,
          fixes: changes.fixes,
        })}
        {extras.map((x) => (
          <span key={x}> &middot; {x}</span>
        ))}
      </div>
      {changes.top_breaking.length > 0 && (
        <ul className="mt-1.5 space-y-1">
          {changes.top_breaking.map((entry, i) => (
            <li key={i} className="text-xs text-text-secondary flex gap-1.5">
              <span className="font-mono text-[10px] text-red-400 shrink-0 mt-px">{entry.version}</span>
              <span>
                {entry.text}
                {entry.under && (
                  <span className="text-text-muted"> ({t('results.releaseChanges.under', { heading: entry.under })})</span>
                )}
              </span>
            </li>
          ))}
        </ul>
      )}
      <div className="mt-1 text-[10px] text-text-muted">
        {t('results.releaseChanges.releases', { versions: changes.versions.join(', ') })}
      </div>
      {!changes.covers_range && (
        <div className="mt-0.5 text-[10px] text-amber-400/80">
          {t('results.releaseChanges.partial', { from: changes.from_version })}
        </div>
      )}
    </>
  );
}

function emptyMessage(changes: ReleaseChanges, t: (k: string, o?: Record<string, unknown>) => string): string {
  switch (changes.status) {
    case 'no_changelog':
      return t('results.releaseChanges.noChangelog');
    case 'unparsed':
      return t('results.releaseChanges.unparsed');
    case 'no_sections_in_range':
      return t('results.releaseChanges.noSectionsInRange', { from: changes.from_version });
    case 'refused':
      return t('results.releaseChanges.refused');
    default:
      return t('results.releaseChanges.unavailable');
  }
}
