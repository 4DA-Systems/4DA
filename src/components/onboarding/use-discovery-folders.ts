// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback, useMemo } from 'react';
import { cmd } from '../../lib/commands';

/** A folder the user may let 4DA scan. Nothing is read until they press Scan. */
export interface FolderChoice {
  path: string;
  checked: boolean;
}

const samePath = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

function mergeFolders(prev: FolderChoice[], paths: readonly string[], checked: boolean): FolderChoice[] {
  const next = [...prev];
  for (const path of paths) {
    if (!next.some(f => samePath(f.path, path))) next.push({ path, checked });
  }
  return next;
}

/**
 * The consent list behind "Scan my projects": the home folders discovery would
 * use (listed, not read), an opt-in search of other drives, and folders the
 * user types in. Only the ticked folders are ever scanned.
 */
export function useDiscoveryFolders() {
  const [folders, setFolders] = useState<FolderChoice[]>([]);
  const [loading, setLoading] = useState(true);
  const [searchingDrives, setSearchingDrives] = useState(false);
  const [drivesSearched, setDrivesSearched] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const found = await cmd('ace_preview_discovery_dirs');
        if (!cancelled) setFolders(prev => mergeFolders(prev, found ?? [], true));
      } catch { /* the user can still add folders by hand */ }
      finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, []);

  const toggleFolder = useCallback((path: string) => {
    setFolders(prev => prev.map(f => (f.path === path ? { ...f, checked: !f.checked } : f)));
  }, []);

  const addFolder = useCallback((raw: string) => {
    const path = raw.trim();
    if (!path) return;
    setFolders(prev => {
      const existing = prev.find(f => samePath(f.path, path));
      if (existing) return prev.map(f => (f === existing ? { ...f, checked: true } : f));
      return [...prev, { path, checked: true }];
    });
  }, []);

  // Opt-in: other drives are only looked at when the user asks, and what is
  // found is offered unticked.
  const findMoreFolders = useCallback(async () => {
    setSearchingDrives(true);
    try {
      const roots = await cmd('ace_candidate_dev_roots');
      setFolders(prev => mergeFolders(prev, roots ?? [], false));
    } catch { /* nothing more to offer */ }
    finally {
      setSearchingDrives(false);
      setDrivesSearched(true);
    }
  }, []);

  const selected = useMemo(() => folders.filter(f => f.checked).map(f => f.path), [folders]);

  return { folders, loading, selected, toggleFolder, addFolder, findMoreFolders, searchingDrives, drivesSearched };
}

export type DiscoveryFolders = ReturnType<typeof useDiscoveryFolders>;
