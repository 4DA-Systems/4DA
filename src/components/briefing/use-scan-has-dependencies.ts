// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useCallback, useEffect, useState } from 'react';

import { cmd } from '../../lib/commands';

/**
 * Whether a project scan has stored dependencies. The "Scan my projects"
 * nudge keyed off declared interests alone, which a scan never fills, so it
 * stayed on the Brief after a completed scan (fresh-profile audit E9). Asked
 * only while `enabled` (the nudge would otherwise show); `recheck` asks again
 * after a scan.
 */
export function useScanHasDependencies(enabled: boolean): [boolean, () => void] {
  const [known, setKnown] = useState(false);
  const [asks, setAsks] = useState(0);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    void cmd('ace_get_scan_summary')
      .then((summary) => {
        const total = (summary as unknown as { total_dependencies?: number } | null)?.total_dependencies ?? 0;
        if (!cancelled) setKnown(total > 0);
      })
      .catch(() => { /* unknown: keep the nudge */ });
    return () => { cancelled = true; };
  }, [enabled, asks]);

  const recheck = useCallback(() => setAsks(n => n + 1), []);
  return [known, recheck];
}
