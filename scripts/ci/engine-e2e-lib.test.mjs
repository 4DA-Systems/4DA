// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Unit tests for the engine end-to-end gate's pure helpers (engine-e2e-lib.mjs).
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, it } from 'node:test';
import { inflateRawSync } from 'node:zlib';

import {
  applyMutation,
  buildZip,
  diffSets,
  etagOf,
  expectedFindingKeys,
  expectedInventory,
  firstDifference,
  groupByEcosystem,
  inventoryByCase,
  MIRROR_ECOSYSTEMS,
  normalizePlan,
  normName,
  normVersion,
} from './engine-e2e-lib.mjs';

describe('frozen archives', () => {
  it('groups records by every ecosystem they affect, with an entry for each known ecosystem', () => {
    const g = groupByEcosystem([
      { name: 'A.json', json: { affected: [{ package: { ecosystem: 'npm' } }, { package: { ecosystem: 'PyPI' } }] } },
      { name: 'B.json', json: { affected: [{ package: { ecosystem: 'SwiftURL' } }] } },
    ]);
    assert.deepEqual(Object.keys(g).sort(), [...MIRROR_ECOSYSTEMS].sort());
    assert.deepEqual(g.npm.map((r) => r.name), ['A.json']);
    assert.deepEqual(g.PyPI.map((r) => r.name), ['A.json']);
    assert.equal(g.Go.length, 0);
  });

  it('writes a ZIP whose entries inflate back to the input, with stable bytes', () => {
    const data = Buffer.from(JSON.stringify({ id: 'GHSA-x', affected: [] }));
    const zip = buildZip([{ name: 'GHSA-x.json', data }]);
    assert.equal(zip.readUInt32LE(0), 0x04034b50);
    const nameLen = zip.readUInt16LE(26);
    const compLen = zip.readUInt32LE(18);
    const body = zip.subarray(30 + nameLen, 30 + nameLen + compLen);
    assert.deepEqual(inflateRawSync(body), data);
    assert.equal(etagOf(buildZip([{ name: 'GHSA-x.json', data }])), etagOf(zip), 'same content, same ETag');
    assert.notEqual(etagOf(buildZip([])), etagOf(zip));
  });

  it('the ZIP opens with a standard reader', (t) => {
    const dir = mkdtempSync(join(tmpdir(), 'e2e-zip-'));
    try {
      const p = join(dir, 'all.zip');
      writeFileSync(p, buildZip([{ name: 'a.json', data: Buffer.from('{"id":"a"}') }, { name: 'b.json', data: Buffer.from('{"id":"b"}') }]));
      let listing;
      try {
        listing = execFileSync('python3', ['-c', 'import sys,zipfile;z=zipfile.ZipFile(sys.argv[1]);print(",".join(z.namelist()));print(z.read("b.json").decode())', p], { encoding: 'utf8' });
      } catch {
        t.skip('python3 not available to cross-check the archive');
        return;
      }
      assert.match(listing, /a\.json,b\.json/);
      assert.match(listing, /"id":"b"/);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe('conformance keys', () => {
  it('normalises names and versions as the corpus README says', () => {
    assert.equal(normName('PyPI', 'Typing_Extensions'), 'typing-extensions');
    assert.equal(normName('Go', 'github.com/BurntSushi/toml'), 'github.com/BurntSushi/toml');
    assert.equal(normName('npm', '@Babel/Core'), '@babel/core');
    assert.equal(normVersion('Go', 'v1.2.3'), '1.2.3');
    assert.equal(normVersion('PyPI', '2.0.0'), '2');
    assert.equal(normVersion('PyPI', '1.0rc1'), '1rc1');
    assert.equal(normVersion('PyPI', 'V2.10.0'), '2.10');
    assert.equal(normVersion('npm', '1.0.0'), '1.0.0');
  });

  it('maps stored rows to case-relative keys and reports rows outside the context root', () => {
    const root = process.platform === 'win32' ? 'C:/w/ctx' : '/w/ctx';
    const rows = [
      { project_path: `${root}/case-a`, ecosystem: 'javascript', package_name: 'Lodash', version: '4.17.20' },
      { project_path: `${root}/case-a/frontend`, ecosystem: 'python', package_name: 'Django_Rest', version: 'v3.0.0' },
      { project_path: '/elsewhere/x', ecosystem: 'rust', package_name: 'serde', version: '1.0.0' },
    ];
    const { byCase, outside } = inventoryByCase(rows, root);
    assert.deepEqual([...byCase['case-a']].sort(), ['PyPI|frontend|django-rest|3', 'npm|.|lodash|4.17.20']);
    assert.deepEqual(outside, ['/elsewhere/x']);
  });

  it('the expected inventory leaves out directories the coverage record names as skipped', () => {
    const expected = {
      inventory: { packages: ['npm|.|a|1', 'npm|benchmarks/x|b|1', 'npm|benchmarks/x/deep|c|1', 'npm|benchmarksish|d|1'] },
      findings: [
        { ecosystem: 'npm', dir: '.', package: 'a', version: '1', ids: ['GHSA-1', 'CVE-1'] },
        { ecosystem: 'npm', dir: 'benchmarks/x', package: 'b', version: '1', ids: ['GHSA-2'] },
      ],
    };
    assert.deepEqual(expectedInventory(expected, ['benchmarks/x']), ['npm|.|a|1', 'npm|benchmarksish|d|1']);
    const f = expectedFindingKeys(expected, ['benchmarks/x']);
    assert.deepEqual([...f.keys()], ['npm|.|a|1']);
    assert.deepEqual([...f.get('npm|.|a|1')[0]].sort(), ['CVE-1', 'GHSA-1']);
  });

  it('applies a lockfile mutation and diffs both ways', () => {
    assert.deepEqual(applyMutation(['b', 'a', 'x'], { removed: ['x'], added: ['c'] }), ['a', 'b', 'c']);
    assert.deepEqual(diffSets(['a', 'b'], ['b', 'c']), { missing: ['c'], extra: ['a'] });
  });
});

describe('plan goldens', () => {
  const root = process.platform === 'win32' ? 'C:/w/ctx' : '/w/ctx';
  const plan = (overrides = {}) => ({
    schema_version: 1,
    generated_at: '2026-11-12T00:00:00Z',
    engine_run_id: 7,
    dependency_inventory_hash: 'abc',
    item_count: 1,
    multi_version_coverage: true,
    validation_drop_count: 0,
    items: [
      {
        id: 'upgrade-plan:npm:x',
        kind: 'alert',
        urgency: 'high',
        title: 'Upgrade x',
        affected_deps: ['x'],
        affected_projects: [`${root}/b`, `${root}/a`],
        evidence: [{ freshness_days: 3.2, relevance_note: `Installed in: ${root}/a/very/long/path/that/gets/cut…` }],
      },
    ],
    steps: [
      {
        item_id: 'upgrade-plan:npm:x',
        package: 'x',
        ecosystem: 'npm',
        mechanism: 'direct',
        advisory_ids: ['GHSA-2', 'GHSA-1'],
        verification: `re-run in ${root}/a`,
        lines: [{ installed: '1.0.0', target: '1.0.1', upgrade_type: 'patch', clears_all_known: true, held: { status: 'unchecked', reason: 'r' }, sites: [{ project: `${root}/a`, dev: true, direct: false }] }],
      },
    ],
    ...overrides,
  });

  it('keeps what the plan says and drops what varies by run, machine or day', () => {
    const n = normalizePlan(plan(), root);
    assert.deepEqual(n.items[0].affected_projects, ['<ctx>/a', '<ctx>/b']);
    assert.deepEqual(n.steps[0].advisory_ids, ['GHSA-1', 'GHSA-2']);
    assert.deepEqual(n.steps[0].lines[0], { installed: '1.0.0', target: '1.0.1', upgrade_type: 'patch', clears_all_known: true, held: 'unchecked', sites: ['<ctx>/a dev'] });
    const text = JSON.stringify(n);
    for (const volatile of ['generated_at', 'engine_run_id', 'dependency_inventory_hash', 'freshness_days', 'verification', root]) {
      assert.ok(!text.includes(volatile), `${volatile} is not in the golden`);
    }
    const later = normalizePlan(plan({ generated_at: '2027-01-01T00:00:00Z', engine_run_id: 99 }), root);
    assert.equal(firstDifference(later, n), null, 'a later run with the same plan matches');
  });

  it('a changed target is a difference, located by path', () => {
    const changed = plan();
    changed.steps[0].lines[0].target = '1.0.2';
    const d = firstDifference(normalizePlan(changed, root), normalizePlan(plan(), root));
    assert.equal(d.path, '$.steps[0].lines[0].target');
    assert.equal(d.actual, '1.0.2');
  });
});
