// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Signal Lane 1 — the stack-change stream (AD-054). The items come from the
// backend `get_stack_changes` command, already ordered by actionability
// (security, yanked, breaking, minor). This file only reads the item contract
// documented in `src-tauri/src/evidence/stack_change.rs`:
//
//   id      stack-change:<change>:...   change = security|yanked|major|breaking|minor
//   evidence[0] = version_context ("crates.io · rmcp 1.7.0 → 2.1.0")
//   suggested_actions with action_id "run_command": label = the exact command,
//   description = where to run it.

import type { EvidenceFeed } from '../../../src-tauri/bindings/bindings/EvidenceFeed';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';

export type StackChange = 'security' | 'yanked' | 'major' | 'breaking' | 'minor';

const CHANGES: readonly StackChange[] = ['security', 'yanked', 'major', 'breaking', 'minor'];

/** Lane 1 rows visible before "Show all N". */
export const STACK_LANE_CAP = 20;

/** The change an item reports, read from its id; null for any other item. */
export function stackChangeOf(item: Pick<EvidenceItem, 'id'>): StackChange | null {
  const [prefix, change] = item.id.split(':');
  if (prefix !== 'stack-change') return null;
  return (CHANGES as readonly string[]).includes(change ?? '') ? (change as StackChange) : null;
}

/** The "rmcp 1.7.0 → 2.1.0" strip, without the ecosystem prefix. */
export function versionLine(item: EvidenceItem): { ecosystem: string; span: string } | null {
  const vc = item.evidence.find((e) => e.source === 'version_context');
  if (!vc) return null;
  const [ecosystem, ...rest] = vc.title.split(' · ');
  return rest.length > 0 ? { ecosystem: ecosystem ?? '', span: rest.join(' · ') } : { ecosystem: '', span: vc.title };
}

export interface StackCommand {
  command: string;
  where: string;
}

/** The exact commands an item names, in order. */
export function commandsOf(item: EvidenceItem): StackCommand[] {
  return item.suggested_actions
    .filter((a) => a.action_id === 'run_command' && a.label.trim() !== '')
    .map((a) => ({ command: a.label, where: a.description.replace(/^Run in /, '') }));
}

/** The link the item's primary action opens: its first cited URL. */
export function primaryLink(item: EvidenceItem): string | null {
  return item.evidence.find((e) => e.url != null && e.url !== '')?.url ?? null;
}

/** What the lane should show for a loaded feed. */
export type StackLaneState = 'cold_start' | 'empty' | 'items';

export function laneState(feed: Pick<EvidenceFeed, 'items' | 'total_tracked'>): StackLaneState {
  // No dependency has been read yet: nothing was compared, so this is
  // onboarding, not "nothing changed" (doctrine rule 6).
  if (feed.total_tracked === 0) return 'cold_start';
  return feed.items.length === 0 ? 'empty' : 'items';
}

/** Releases were left out because they are part of Signal (AD-054 rule 6). */
export function releasesWithheld(feed: Pick<EvidenceFeed, 'tier_scope'>): boolean {
  return feed.tier_scope === 'free_floor';
}
