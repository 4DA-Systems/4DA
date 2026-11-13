// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { scanOutcome } from './scan-outcome';
import type { DiscoveryResult } from '../../lib/discovery';
import type { ScanSummary } from '../first-run/utils';

const result = (over: Partial<DiscoveryResult> = {}): DiscoveryResult => ({
  success: true, directories_found: 2, projects_found: 0, directories_added: 2, directories: [],
  scan_result: {
    manifest_scan: { detected_tech: 0, confidence: 0 },
    git_scan: { repos_analyzed: 0, total_commits: 0 },
    combined: { total_topics: 0, topics: [] },
  },
  ...over,
});

const summary = (deps: ScanSummary['dependencies_by_ecosystem']): ScanSummary => ({
  projects_scanned: 2,
  total_dependencies: deps.npm + deps.rust + deps.python + deps.other,
  dependencies_by_ecosystem: deps,
  languages: [], frameworks: [], primary_stack: '', key_packages: [], has_data: true,
});

describe('scanOutcome', () => {
  it('counts per ecosystem, leaving out the empty ones', () => {
    const out = scanOutcome(result({ projects_found: 2 }), summary({ npm: 900, rust: 0, python: 12, other: 3 }));
    expect(out).toEqual({
      kind: 'found', projects: 2, dependencies: 915,
      ecosystems: [{ ecosystem: 'npm', count: 900 }, { ecosystem: 'python', count: 12 }, { ecosystem: 'other', count: 3 }],
    });
  });

  it('is "none" when nothing was found, never a silent success', () => {
    expect(scanOutcome(result(), null)).toEqual({ kind: 'none', directories: 2 });
    expect(scanOutcome(result({ status: 'no_directories', success: false }), null).kind).toBe('none');
  });

  it('is an error when the scan failed', () => {
    expect(scanOutcome(result({ success: false, message: 'Access denied' }), null))
      .toEqual({ kind: 'error', message: 'Access denied' });
  });
});
