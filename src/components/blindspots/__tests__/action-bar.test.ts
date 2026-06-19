// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { bareName, depEcosystem, investigateUrl } from '../StackCoverageMap';
import type { DepRow } from '../types';

// Phase A: the grouped "Investigate" action builds a registry/search URL from a
// dependency's name + ecosystem. These helpers back the always-visible action bar.

function row(name: string, gapId: string | null): DepRow {
  return {
    name,
    status: 'blind_spot',
    urgency: 'medium',
    gap: gapId ? ({ id: gapId, affected_deps: [bareName(name)] } as unknown as DepRow['gap']) : null,
    signals: [],
    projects: [],
  };
}

describe('bareName', () => {
  it('strips the ecosystem qualifier', () => {
    expect(bareName('ammonia (crates.io)')).toBe('ammonia');
    expect(bareName('@sentry/node (npm)')).toBe('@sentry/node');
    expect(bareName('typescript')).toBe('typescript');
  });
});

describe('depEcosystem', () => {
  it('reads the ecosystem from the "(eco)" suffix', () => {
    expect(depEcosystem(row('axum (crates.io)', null))).toBe('crates.io');
    expect(depEcosystem(row('react (npm)', null))).toBe('npm');
  });
  it('falls back to the bs_uncov_<eco>_ id prefix', () => {
    expect(depEcosystem(row('mystery', 'bs_uncov_pypi_mystery'))).toBe('pypi');
  });
});

describe('investigateUrl', () => {
  it('routes each ecosystem to its registry', () => {
    expect(investigateUrl('axum', 'crates.io')).toBe('https://crates.io/crates/axum');
    expect(investigateUrl('react', 'npm')).toBe('https://www.npmjs.com/package/react');
    expect(investigateUrl('requests', 'python')).toBe('https://pypi.org/project/requests');
    expect(investigateUrl('gin', 'go')).toBe('https://pkg.go.dev/gin');
  });
  it('falls back to a web search for unknown ecosystems', () => {
    expect(investigateUrl('foo', 'maven')).toContain('https://www.google.com/search?q=');
    expect(investigateUrl('foo', 'maven')).toContain(encodeURIComponent('foo maven'));
  });
  it('url-encodes scoped npm names', () => {
    expect(investigateUrl('@sentry/node', 'npm')).toBe('https://www.npmjs.com/package/%40sentry%2Fnode');
  });
});
