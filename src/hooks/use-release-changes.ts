// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useState } from 'react';
import { cmd } from '../lib/commands';
import type { ReleaseChanges } from '../../src-tauri/bindings/bindings/ReleaseChanges';

/** Registry sources whose rows can carry a graded release. */
const RELEASE_SOURCES = new Set(['crates_io', 'crates', 'npm_registry', 'npm']);

export function isRegistryReleaseSource(sourceType: string | undefined): boolean {
  return sourceType !== undefined && RELEASE_SOURCES.has(sourceType);
}

export interface ReleaseChangesState {
  changes: ReleaseChanges | null;
  loading: boolean;
}

/**
 * "What changed" for a graded registry release, fetched when the card is
 * expanded. `null` for any item the backend does not grade as news.
 */
export function useReleaseChanges(itemId: number, sourceType: string | undefined, enabled: boolean): ReleaseChangesState {
  const [changes, setChanges] = useState<ReleaseChanges | null>(null);
  const [loading, setLoading] = useState(false);
  const applies = enabled && isRegistryReleaseSource(sourceType);

  useEffect(() => {
    if (!applies) return;
    let cancelled = false;
    setLoading(true);
    void cmd('get_release_changes', { itemId })
      .then((result) => {
        if (cancelled) return;
        setChanges(result);
        setLoading(false);
      })
      .catch((e: unknown) => {
        console.warn('[4DA] Release changes fetch failed:', e);
        if (cancelled) return;
        setChanges(null);
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [applies, itemId]);

  return { changes: applies ? changes : null, loading: applies && loading };
}
