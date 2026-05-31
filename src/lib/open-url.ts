// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Open a URL in the user's default browser via the Tauri opener plugin, with a
 * web `window.open` fallback. This mirrors the pattern duplicated across the
 * briefing, feed, alerts, and graph surfaces — the canonical "open this item".
 */
export function openExternalUrl(url: string): void {
  if (!url) return;
  void import('@tauri-apps/plugin-opener')
    .then(({ openUrl }) => openUrl(url))
    .catch(() => {
      window.open(url, '_blank', 'noopener,noreferrer');
    });
}
