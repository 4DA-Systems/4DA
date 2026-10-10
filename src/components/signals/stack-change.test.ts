// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import { commandsOf, laneState, primaryLink, releasesWithheld, stackChangeOf, versionLine } from './stack-change';

const base = {
  kind: 'alert', title: 't', explanation: '', confidence: { value: 0.9, provenance: 'checklist' }, urgency: 'watch',
  affected_projects: [], affected_deps: ['serde'], lens_hints: {}, created_at: 0,
} as const;

function item(id: string, extra: Partial<EvidenceItem> = {}): EvidenceItem {
  return { ...base, id, evidence: [], suggested_actions: [], ...extra } as unknown as EvidenceItem;
}

describe('stack-change item contract', () => {
  it('reads the change from the id and nothing else', () => {
    expect(stackChangeOf(item('stack-change:security:crates.io:rmcp:4da/src-tauri'))).toBe('security');
    expect(stackChangeOf(item('stack-change:breaking:crates.io:lopdf@0.45.0'))).toBe('breaking');
    expect(stackChangeOf(item('stack-change:minor:npm:@scope/x@1.2.0'))).toBe('minor');
    expect(stackChangeOf(item('stack-change:patch:npm:x@1.0.1'))).toBeNull();
    expect(stackChangeOf(item('upgrade-plan:rmcp'))).toBeNull();
  });

  it('splits the version strip into ecosystem and span', () => {
    const it1 = item('stack-change:minor:npm:x@1.2.0', {
      evidence: [{ source: 'version_context', title: 'npm · x 1.0.0 → 1.2.0', freshness_days: 0 }],
    } as Partial<EvidenceItem>);
    expect(versionLine(it1)).toEqual({ ecosystem: 'npm', span: 'x 1.0.0 → 1.2.0' });
    expect(versionLine(item('stack-change:minor:npm:x@1.2.0'))).toBeNull();
  });

  it('lists only run_command actions, with where to run them', () => {
    const it1 = item('stack-change:minor:crates.io:serde@1.2.0', {
      suggested_actions: [
        { action_id: 'review_updates', label: 'Review the release', description: 'Open' },
        { action_id: 'run_command', label: 'cargo update -p serde@1.0.0 --precise 1.2.0', description: 'Run in app/src-tauri' },
      ],
    });
    expect(commandsOf(it1)).toEqual([{ command: 'cargo update -p serde@1.0.0 --precise 1.2.0', where: 'app/src-tauri' }]);
  });

  it('opens the first cited URL', () => {
    const it1 = item('stack-change:security:x', {
      evidence: [
        { source: 'version_context', title: 'npm · x', freshness_days: 0 },
        { source: 'osv', title: 'GHSA-1', url: 'https://osv.dev/vulnerability/GHSA-1', freshness_days: 0 },
      ],
    } as Partial<EvidenceItem>);
    expect(primaryLink(it1)).toBe('https://osv.dev/vulnerability/GHSA-1');
  });

  it('tells cold start (nothing read) from "nothing changed"', () => {
    expect(laneState({ items: [], total_tracked: 0 })).toBe('cold_start');
    expect(laneState({ items: [], total_tracked: 40 })).toBe('empty');
    expect(laneState({ items: [], total_tracked: null })).toBe('empty');
    expect(laneState({ items: [item('stack-change:minor:npm:x@1.2.0')], total_tracked: 40 })).toBe('items');
    expect(releasesWithheld({ tier_scope: 'free_floor' })).toBe(true);
    expect(releasesWithheld({ tier_scope: 'full' })).toBe(false);
  });
});
