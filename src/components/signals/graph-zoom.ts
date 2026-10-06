// SPDX-License-Identifier: FSL-1.1-Apache-2.0

/**
 * A size that holds constant ON SCREEN at every zoom below 1: it divides by
 * the live viewport zoom (`--graph-zoom`, written by ZoomCssVar) and never
 * enlarges past zoom 1. At fit view a 150-node map sits near zoom 0.25, where
 * the old fixed 10px node labels rendered ~2.5px tall (audit 2026-10-02).
 */
export const zoomInvariant = (px: number): string =>
  `calc(${px}px / min(var(--graph-zoom, 1), 1))`;
