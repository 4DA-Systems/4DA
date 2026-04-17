// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Global keyboard shortcut hook for the Confession Box.
 *
 * Listens for Cmd+Period (macOS) / Ctrl+Period (everyone else) and
 * invokes the provided callback. Escape-key close is wired by the
 * modal itself, not here.
 *
 * Intelligence Reconciliation Phase 10 (2026-04-17).
 */

import { useEffect } from 'react';

export function useConfessionShortcut(onTrigger: () => void): void {
  useEffect(() => {
    function handler(e: KeyboardEvent) {
      // The "." key matches both Cmd+. and Ctrl+. across layouts.
      if (e.key !== '.' && e.code !== 'Period') return;
      if (!e.ctrlKey && !e.metaKey) return;
      // Don't hijack the shortcut when focus is inside another modal's
      // form element that might need Ctrl+. for its own behavior.
      const target = e.target as HTMLElement | null;
      const tag = target?.tagName?.toLowerCase();
      if (tag === 'textarea' || tag === 'input') {
        // Allow Ctrl+. to open the Confession Box from anywhere INCLUDING
        // input fields — the whole point of this shortcut is instant
        // access regardless of focus. But let's give it one exception:
        // if the user is already inside the Confession Box's own input,
        // don't re-trigger. The modal's onOpenChange handles that.
        if (target?.dataset.confessionInput === 'true') return;
      }
      e.preventDefault();
      onTrigger();
    }
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [onTrigger]);
}
