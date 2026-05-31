// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Canonical types for the global command search.
 *
 * Every source of results — navigation, actions, intelligence — emits the
 * SAME `CommandResult` shape, and the dropdown renders that shape blindly.
 * New searchable surfaces are added by registering a `SearchProvider`, never
 * by special-casing the renderer. This mirrors the project's intelligence
 * doctrine: one canonical type, one entry point.
 */

/** Result categories, rendered as labelled sections in display order. */
export type CommandGroup = 'goto' | 'action' | 'intelligence';

export const GROUP_ORDER: readonly CommandGroup[] = ['goto', 'action', 'intelligence'];

export interface CommandResult {
  /** Stable unique id (used as React key and for de-duplication). */
  id: string;
  group: CommandGroup;
  /** Primary label shown to the user. */
  title: string;
  /** Optional secondary line (description, file path, match reason). */
  subtitle?: string;
  /** 0..1 relevance, used to rank within a group. Deterministic tiers use 1. */
  score: number;
  /** Optional trailing badge text (e.g. a relevance pill or "Signal"). */
  badge?: string;
  /** Invoked when the row is selected. Must not throw. */
  run: () => void;
}

export interface ProviderContext {
  /** Trimmed query text. May be empty (launcher mode). */
  query: string;
  /** Aborts when a newer keystroke supersedes this run. Async providers MUST honor it. */
  signal: AbortSignal;
}

export interface SearchProvider {
  id: string;
  group: CommandGroup;
  /**
   * `sync` providers resolve instantly on the main thread (nav, actions) and
   * carry the experience even when the backend is slow or offline. `async`
   * providers hit the Rust backend and stream in additively.
   */
  kind: 'sync' | 'async';
  query(ctx: ProviderContext): CommandResult[] | Promise<CommandResult[]>;
}

/**
 * Lightweight subsequence fuzzy scorer. Returns 0..1 (higher = better) or -1
 * for no match. Exact prefix > word-boundary > scattered subsequence.
 */
export function fuzzyScore(query: string, text: string): number {
  const q = query.trim().toLowerCase();
  const t = text.toLowerCase();
  if (q.length === 0) return 0.5; // launcher mode: everything weakly matches
  if (t === q) return 1;
  if (t.startsWith(q)) return 0.95;
  const wordStart = t.includes(` ${q}`) || t.split(/[\s\-_/.]+/).some(w => w.startsWith(q));
  if (t.includes(q)) return wordStart ? 0.85 : 0.7;

  // Scattered subsequence: all query chars appear in order.
  let ti = 0;
  let matched = 0;
  for (let qi = 0; qi < q.length; qi++) {
    const found = t.indexOf(q.charAt(qi), ti);
    if (found === -1) return -1;
    ti = found + 1;
    matched++;
  }
  // Density bonus: tighter spans score higher.
  return 0.3 + 0.25 * (matched / Math.max(t.length, 1));
}
