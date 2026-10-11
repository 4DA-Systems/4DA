// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';

import { dependencyProvider, matchDependencyAlerts, queryTokens } from './dependency-search-provider';
import type { EvidenceFeed } from '../../../src-tauri/bindings/bindings/EvidenceFeed';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import type { Urgency } from '../../../src-tauri/bindings/bindings/Urgency';
import type { CommandResult } from './command-search-types';

function alert(id: string, deps: string[], urgency: Urgency, projects = ['d:/runyourempire/navcal']): EvidenceItem {
  return {
    id,
    kind: 'alert',
    title: `Upgrade ${deps[0]} — clears 1 advisory`,
    explanation: '',
    confidence: { value: 0.9, provenance: 'heuristic' },
    urgency,
    evidence: [],
    affected_projects: projects,
    affected_deps: deps,
    suggested_actions: [],
    lens_hints: { preemption: true },
    created_at: 0n,
  } as unknown as EvidenceItem;
}

// Shapes taken from the live feed on 2026-10-11 (the audit that motivated this group).
const ITEMS = [
  alert('upgrade-plan:npm:sharp', ['sharp'], 'high'),
  alert('upgrade-plan:npm:next', ['next'], 'medium'),
  alert('upgrade-plan:npm:@ai-sdk/provider-utils', ['@ai-sdk/provider-utils'], 'medium'),
  alert('upgrade-plan:crates.io:serde_with', ['serde_with'], 'medium', ['d:/runyourempire/victauri']),
  alert('upgrade-plan:npm:request', ['request'], 'medium'),
  alert('upgrade-plan:npm:react-dom', ['react-dom'], 'watch'),
];

const feed = (items: EvidenceItem[]): EvidenceFeed => ({ items, total: items.length, critical_count: 0, high_count: 0, score: null });

const ids = (q: string) => matchDependencyAlerts(ITEMS, q).map(m => m.item.id);

describe('queryTokens', () => {
  it('reads the forms a package is typed in', () => {
    expect(queryTokens('next.js security advisory')[0]).toEqual(['next.js', 'next']);
    expect(queryTokens('nextjs')[0]).toEqual(['nextjs', 'next']);
    expect(queryTokens('sharp@0.33')[0]).toEqual(['sharp']);
    expect(queryTokens('@ai-sdk/provider-utils@3.0.15')[0]).toEqual(['@ai-sdk/provider-utils']);
    expect(queryTokens('sharp?')[0]).toEqual(['sharp']);
  });

  it('drops one-letter words and empty input', () => {
    expect(queryTokens('a')).toEqual([]);
    expect(queryTokens('   ')).toEqual([]);
  });
});

describe('matchDependencyAlerts', () => {
  it('finds the advisory a package query names (the audit queries)', () => {
    expect(ids('sharp vulnerability')).toEqual(['upgrade-plan:npm:sharp']);
    expect(ids('next.js security advisory')).toEqual(['upgrade-plan:npm:next']);
  });

  it('matches a scoped package by its bare name, and crates across _ and -', () => {
    expect(ids('provider-utils')).toEqual(['upgrade-plan:npm:@ai-sdk/provider-utils']);
    expect(ids('serde-with')).toEqual(['upgrade-plan:crates.io:serde_with']);
  });

  it('a single partial word of 3+ chars matches by prefix, ranked below an exact hit', () => {
    expect(ids('shar')).toEqual(['upgrade-plan:npm:sharp']);
    const [m] = matchDependencyAlerts(ITEMS, 'shar');
    const [exact] = matchDependencyAlerts(ITEMS, 'sharp');
    expect(m!.score).toBeLessThan(exact!.score);
  });

  it('negatives: no prefix match for 2 chars, inside a phrase, or a mid-word substring', () => {
    expect(ids('re')).toEqual([]); // not request / react-dom
    expect(ids('react hooks')).toEqual([]); // "react" is not react-dom, and phrases match whole words only
    expect(ids('harp')).toEqual([]); // substring of sharp, not a prefix
    expect(ids('tokio runtime upgrade')).toEqual([]);
  });

  it('an exact hit outranks a prefix hit of higher urgency (live feed: "vite" listed vitest first)', () => {
    const items = [alert('vitest', ['vitest'], 'critical'), alert('vite', ['vite'], 'watch')];
    expect(matchDependencyAlerts(items, 'vite').map(m => m.item.id)).toEqual(['vite', 'vitest']);
  });

  it('ranks by urgency', () => {
    const order = matchDependencyAlerts(ITEMS, 'next sharp').map(m => m.item.id);
    expect(order).toEqual(['upgrade-plan:npm:sharp', 'upgrade-plan:npm:next']);
  });
});

describe('dependencyProvider', () => {
  const t = (key: string, fallback?: string) => fallback ?? key;
  // The provider is sync: its query returns the rows, never a promise.
  const run = (p: ReturnType<typeof dependencyProvider>, query: string) =>
    p.query({ query, signal: new AbortController().signal }) as CommandResult[];

  it('yields nothing before the feed has loaded', () => {
    const p = dependencyProvider({ t, openPreemption: vi.fn(), getPreemptionFeed: () => null });
    expect(run(p, 'sharp')).toEqual([]);
  });

  it('rows carry the alert title, project names and urgency, and open the worklist', () => {
    const openPreemption = vi.fn();
    const p = dependencyProvider({ t, openPreemption, getPreemptionFeed: () => feed(ITEMS) });
    const [row, ...rest] = run(p, 'sharp vulnerability');
    expect(rest).toEqual([]);
    expect(row).toMatchObject({
      id: 'dep-upgrade-plan:npm:sharp',
      group: 'dependency',
      title: 'Upgrade sharp — clears 1 advisory',
      subtitle: 'runyourempire/navcal',
      badge: 'high',
    });
    row!.run();
    expect(openPreemption).toHaveBeenCalledWith('worklist');
  });

  it('reads the feed at query time, so a feed loaded after the provider was built is used', () => {
    let current: EvidenceFeed | null = null;
    const p = dependencyProvider({ t, openPreemption: vi.fn(), getPreemptionFeed: () => current });
    expect(run(p, 'sharp')).toEqual([]);
    current = feed(ITEMS);
    expect(run(p, 'sharp')).toHaveLength(1);
  });

  it("two repos' Tauri crates are told apart (live feed: glib read 'src-tauri, src-tauri, victauri')", () => {
    const glib = alert('glib', ['glib'], 'watch', [
      'd:/4da/src-tauri',
      'd:/work/tools/apps/bridge/src-tauri',
      'd:/runyourempire/victauri',
    ]);
    const p = dependencyProvider({ t, openPreemption: vi.fn(), getPreemptionFeed: () => feed([glib]) });
    expect(run(p, 'glib')[0]!.subtitle).toBe('4da/src-tauri, bridge/src-tauri, runyourempire/victauri');
  });

  it('caps the group at four rows', () => {
    const many = Array.from({ length: 7 }, (_, i) => alert(`a${i}`, ['lodash'], 'medium'));
    const p = dependencyProvider({ t, openPreemption: vi.fn(), getPreemptionFeed: () => feed(many) });
    expect(run(p, 'lodash')).toHaveLength(4);
  });
});
