// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Tests for the standalone briefing window script (public/briefing.js). The
// script is plain browser JS with no exports, so its pure builders are pulled
// out by evaluating the source in a function scope (jsdom provides `document`;
// init() bails out without the Tauri globals).
import { describe, expect, it, vi } from 'vitest';

import source from '../../public/briefing.js?raw';

type Df = Record<string, number | boolean | string | null>;
interface Builders {
  buildFreshnessLine: (df: Df | null) => string;
  buildPersistentStrip: (alerts: Record<string, unknown>[]) => string;
}

function loadBuilders(): Builders {
  const spy = vi.spyOn(console, 'error').mockImplementation(() => undefined);
  // The script wires listeners on its window's elements at load; give it some.
  const byId = vi
    .spyOn(document, 'getElementById')
    .mockImplementation(() => document.createElement('div'));
  try {
    return new Function(
      `${source}\nreturn { buildFreshnessLine: buildFreshnessLine, buildPersistentStrip: buildPersistentStrip };`,
    )() as Builders;
  } finally {
    byId.mockRestore();
    spy.mockRestore();
  }
}

const { buildFreshnessLine, buildPersistentStrip } = loadBuilders();

const receipt = (over: Df = {}): Df => ({
  last_run_items_scored: 104,
  last_run_sources_succeeded: 6,
  last_run_sources_failed: 0,
  last_run_sources_skipped: 16,
  last_run_age_minutes: 6,
  newest_source_check_age_hours: 0.1,
  ...over,
});

describe('buildFreshnessLine', () => {
  it('shows ok/total sources for a cycle that fetched', () => {
    expect(buildFreshnessLine(receipt())).toBe('Scanned 104 · 6/6 sources · 6m ago');
  });

  it('reads a cache-fresh no-op cycle as fresh, not as 0 sources (live 2026-10-24)', () => {
    const df = receipt({
      last_run_sources_succeeded: 0,
      last_run_sources_skipped: 22,
      newest_source_check_age_hours: 12 / 60,
    });
    expect(buildFreshnessLine(df)).toBe('Scanned 104 · sources fresh (cached) · 6m ago');
  });

  it('keeps 0 sources when the skips were not the recent-fetch cache', () => {
    // Every source disabled or circuit-open: no source succeeded near the run.
    const df = receipt({
      last_run_sources_succeeded: 0,
      last_run_sources_skipped: 22,
      newest_source_check_age_hours: 30,
    });
    expect(buildFreshnessLine(df)).toBe('Scanned 104 · 0 sources · 6m ago');
  });

  it('keeps 0 sources for an old payload without the skipped count', () => {
    const df = receipt({ last_run_sources_succeeded: 0, last_run_sources_skipped: null });
    expect(buildFreshnessLine(df)).toBe('Scanned 104 · 0 sources · 6m ago');
  });

  it('falls back to the item watermark without a receipt', () => {
    expect(buildFreshnessLine({ last_run_items_scored: null, newest_item_age_hours: 2 })).toBe(
      'Freshest item 2h ago',
    );
  });
});

describe('buildPersistentStrip', () => {
  it('dates a group by its oldest first sighting, not as "unchanged since"', () => {
    const html = buildPersistentStrip([
      { package_name: 'tokio', first_seen_date: '2026-10-20' },
      { package_name: 'zod', first_seen_date: '2026-10-12' },
    ]);
    expect(html).toContain('2 known advisories still open, oldest first seen 12 Oct');
    expect(html).not.toContain('unchanged since');
  });

  it('uses "first seen" for a single advisory', () => {
    const html = buildPersistentStrip([{ package_name: 'tokio', first_seen_date: '2026-10-20' }]);
    expect(html).toContain('1 known advisory still open, first seen 20 Oct');
  });

  it('omits the date when none is known', () => {
    const html = buildPersistentStrip([{ package_name: 'tokio' }]);
    expect(html).toContain('1 known advisory still open<');
  });
});
