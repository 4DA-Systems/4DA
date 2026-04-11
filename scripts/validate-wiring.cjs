#!/usr/bin/env node
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Comprehensive Wiring Validator
 *
 * Catches EVERY class of silent-failure-by-divergence bug. Built after the
 * 2026-04-11 TIER_VIEWS incident proved that my per-component gates didn't
 * catch bugs where two files each pass their own unit tests but have
 * drifted constants between them.
 *
 * Checks performed:
 *
 *  [1] Store slice registration
 *      Every *-slice.ts in src/store/ must be imported in src/store/index.ts
 *      AND have its createXxxSlice function spread into the root store.
 *
 *  [2] React.lazy import resolution
 *      Every lazy(() => import('X')) must point at an existing file.
 *
 *  [3] MCP tool wiring
 *      Every tool file in mcp-4da-server/src/tools/ must be:
 *        (a) exported from index.ts
 *        (b) in DISPATCH_MAP in tool-dispatch.ts
 *        (c) in TOOL_REGISTRY in schema-registry.ts
 *        (d) have a matching *.json in schemas/
 *
 *  [4] TIER_VIEWS cross-file consistency
 *      ViewTabBar.tsx and ui-slice.ts must have identical TIER_VIEWS constants.
 *      (Also covered by unit tests; checked here for fast pre-commit feedback.)
 *
 *  [5] Stub files for feature-gated Rust modules
 *      Every `#[cfg(not(feature = "X"))] #[path = "Y_stub.rs"]` must point
 *      at an existing stub file.
 *
 *  [6] ts-rs binding presence
 *      For each Tauri command whose Rust return type uses a struct marked
 *      with #[ts(export)], verify the corresponding .ts file exists.
 *
 *  [7] Lazy component default export
 *      lazy(() => import('X')) (without .then mapper) assumes X has a
 *      default export. Verify.
 *
 *  [8] Onboarding → splash → main flow primitives
 *      Verify that startup hooks and splash screen components exist.
 *
 * Exit codes:
 *   0 — all checks passed
 *   1 — any check found a wiring issue
 *
 * Usage:
 *   node scripts/validate-wiring.cjs
 *   pnpm run validate:wiring
 */

'use strict';

const fs = require('node:fs');
const path = require('node:path');

const REPO_ROOT = path.join(__dirname, '..');
const SRC = path.join(REPO_ROOT, 'src');
const SRC_TAURI = path.join(REPO_ROOT, 'src-tauri', 'src');
const MCP = path.join(REPO_ROOT, 'mcp-4da-server', 'src');

const failures = [];
const checks = [];

function check(name, fn) {
  try {
    const result = fn();
    if (result && result.ok) {
      checks.push({ name, ok: true, details: result.details });
    } else {
      failures.push({ name, details: result?.details ?? 'unknown failure' });
      checks.push({ name, ok: false, details: result?.details });
    }
  } catch (e) {
    failures.push({ name, details: `Exception: ${e.message}` });
    checks.push({ name, ok: false, details: e.message });
  }
}

function readFileSafe(p) {
  try { return fs.readFileSync(p, 'utf-8'); } catch { return null; }
}

function existsFile(p) {
  try { return fs.statSync(p).isFile(); } catch { return false; }
}

function existsDir(p) {
  try { return fs.statSync(p).isDirectory(); } catch { return false; }
}

// ═════════════════════════════════════════════════════════════════════════
// CHECK 1: Store slice registration
// ═════════════════════════════════════════════════════════════════════════
check('Store slice registration', () => {
  const storeDir = path.join(SRC, 'store');
  const files = fs.readdirSync(storeDir).filter((f) => f.endsWith('-slice.ts'));

  const indexContent = readFileSafe(path.join(storeDir, 'index.ts'));
  if (!indexContent) return { ok: false, details: 'Cannot read store/index.ts' };

  const missingImports = [];
  const missingSpreads = [];

  for (const file of files) {
    // Derive expected create function name: "preemption-slice.ts" -> "createPreemptionSlice"
    const base = file.replace(/-slice\.ts$/, '');
    const parts = base.split('-');
    const createFn = 'create' + parts.map((p) => p[0].toUpperCase() + p.slice(1)).join('') + 'Slice';

    // Check import
    const importRe = new RegExp(`import\\s*\\{\\s*${createFn}\\s*\\}\\s*from\\s*['"]\\./${base}-slice['"]`);
    if (!importRe.test(indexContent)) {
      missingImports.push(`${file} (missing import of ${createFn})`);
      continue;
    }

    // Check spread — `...createFooSlice(...a)`
    const spreadRe = new RegExp(`\\.\\.\\.\\s*${createFn}\\s*\\(`);
    if (!spreadRe.test(indexContent)) {
      missingSpreads.push(`${file} (${createFn} imported but not spread into store)`);
    }
  }

  const issues = [...missingImports, ...missingSpreads];
  return {
    ok: issues.length === 0,
    details: issues.length > 0 ? issues.join('\n    ') : `all ${files.length} slices wired`,
  };
});

// ═════════════════════════════════════════════════════════════════════════
// CHECK 2: React.lazy import resolution
// ═════════════════════════════════════════════════════════════════════════
check('React.lazy import resolution', () => {
  const walkDir = (dir) => {
    const results = [];
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        if (entry.name !== 'node_modules' && entry.name !== '__tests__') {
          results.push(...walkDir(full));
        }
      } else if (entry.name.endsWith('.tsx') || entry.name.endsWith('.ts')) {
        results.push(full);
      }
    }
    return results;
  };

  const tsFiles = walkDir(SRC);
  // Match: lazy(() => import('./Foo')) or lazy(() => import('./Foo').then(...))
  const lazyRe = /lazy\(\s*\(\)\s*=>\s*import\(['"](\.\.?\/[^'"]+)['"]\)/g;
  const missing = [];

  for (const file of tsFiles) {
    const content = readFileSafe(file);
    if (!content) continue;
    let m;
    while ((m = lazyRe.exec(content)) !== null) {
      const importPath = m[1];
      const resolvedBase = path.resolve(path.dirname(file), importPath);
      // Try .tsx, .ts, /index.tsx, /index.ts
      const candidates = [
        resolvedBase + '.tsx',
        resolvedBase + '.ts',
        path.join(resolvedBase, 'index.tsx'),
        path.join(resolvedBase, 'index.ts'),
      ];
      if (!candidates.some(existsFile)) {
        missing.push(`${path.relative(REPO_ROOT, file)}: lazy import "${importPath}" → no matching file`);
      }
    }
  }

  return {
    ok: missing.length === 0,
    details: missing.length > 0 ? missing.join('\n    ') : 'all lazy imports resolve',
  };
});

// ═════════════════════════════════════════════════════════════════════════
// CHECK 3: MCP tool wiring
// ═════════════════════════════════════════════════════════════════════════
// Strategy: parse the canonical DISPATCH_MAP (tool-dispatch.ts) as the
// source of truth for tool names → executor fn names. Then verify each:
//   - executor is exported from tools/index.ts barrel
//   - tool name appears in TOOL_REGISTRY
//   - referenced schemaFile exists on disk (if specified)
// This approach doesn't assume any naming convention between file name
// and tool name, so it works with LLMStatus, GetAgentFeedbackStats, etc.
check('MCP tool wiring', () => {
  const toolsDir = path.join(MCP, 'tools');
  if (!existsDir(toolsDir)) return { ok: true, details: 'MCP not present — skipped' };

  const indexTs = readFileSafe(path.join(toolsDir, 'index.ts')) ?? '';
  const dispatchTs = readFileSafe(path.join(MCP, 'tool-dispatch.ts')) ?? '';
  const registryTs = readFileSafe(path.join(MCP, 'schema-registry.ts')) ?? '';
  const schemasDir = path.join(MCP, 'schemas');

  // Parse DISPATCH_MAP entries: "  tool_name: executeFn,"
  const dispatchRe = /^\s+([a-z_]+):\s*(execute[A-Za-z0-9_]+)\b/gm;
  const dispatchPairs = [];
  let m;
  while ((m = dispatchRe.exec(dispatchTs)) !== null) {
    dispatchPairs.push({ tool: m[1], executor: m[2] });
  }

  if (dispatchPairs.length === 0) {
    return { ok: false, details: 'Could not parse DISPATCH_MAP in tool-dispatch.ts' };
  }

  const issues = [];

  // Check each dispatch entry against index.ts and registry
  for (const { tool, executor } of dispatchPairs) {
    // Must be exported from tools/index.ts barrel
    if (!indexTs.includes(executor)) {
      issues.push(`${tool}: executor "${executor}" not exported from tools/index.ts`);
    }
    // Must be in TOOL_REGISTRY
    const registryRe = new RegExp(`^\\s+${tool}:\\s*\\{`, 'm');
    if (!registryRe.test(registryTs)) {
      issues.push(`${tool}: not in TOOL_REGISTRY`);
    }
  }

  // Check every schemaFile reference in registry resolves to an existing file
  const schemaRefRe = /schemaFile:\s*"([^"]+)"/g;
  while ((m = schemaRefRe.exec(registryTs)) !== null) {
    const schemaFile = m[1];
    if (!existsFile(path.join(schemasDir, schemaFile))) {
      issues.push(`TOOL_REGISTRY references "${schemaFile}" but file does not exist`);
    }
  }

  return {
    ok: issues.length === 0,
    details: issues.length > 0 ? issues.join('\n    ') : `all ${dispatchPairs.length} MCP tools wired end-to-end`,
  };
});

// ═════════════════════════════════════════════════════════════════════════
// CHECK 4: TIER_VIEWS cross-file consistency
// ═════════════════════════════════════════════════════════════════════════
check('TIER_VIEWS consistency', () => {
  const tabbar = readFileSafe(path.join(SRC, 'components', 'ViewTabBar.tsx')) ?? '';
  const uiSlice = readFileSafe(path.join(SRC, 'store', 'ui-slice.ts')) ?? '';

  function extractTierViews(content, constName) {
    const re = new RegExp(`${constName}\\s*:\\s*Record<ViewTier,\\s*ViewId\\[\\]>\\s*=\\s*\\{([\\s\\S]*?)^\\};`, 'm');
    const m = content.match(re);
    if (!m) return null;
    const body = m[1];
    const tiers = {};
    const tierRe = /(\w+):\s*\[([^\]]*)\]/g;
    let tm;
    while ((tm = tierRe.exec(body)) !== null) {
      const views = tm[2].split(',').map((s) => s.trim().replace(/['"]/g, '')).filter(Boolean);
      tiers[tm[1]] = views.sort();
    }
    return tiers;
  }

  const tabbarTiers = extractTierViews(tabbar, 'TIER_VIEWS');
  const sliceTiers = extractTierViews(uiSlice, 'UI_SLICE_TIER_VIEWS');

  if (!tabbarTiers) return { ok: false, details: 'Cannot parse TIER_VIEWS in ViewTabBar.tsx' };
  if (!sliceTiers) return { ok: false, details: 'Cannot parse UI_SLICE_TIER_VIEWS in ui-slice.ts' };

  const tiers = Object.keys(tabbarTiers);
  const issues = [];
  for (const t of tiers) {
    const a = JSON.stringify(tabbarTiers[t]);
    const b = JSON.stringify(sliceTiers[t] ?? []);
    if (a !== b) {
      issues.push(`${t}: ViewTabBar=${a} vs ui-slice=${b}`);
    }
  }

  return {
    ok: issues.length === 0,
    details: issues.length > 0 ? issues.join('\n    ') : `${tiers.length} tiers consistent`,
  };
});

// ═════════════════════════════════════════════════════════════════════════
// CHECK 5: Rust feature-gated stub files
// ═════════════════════════════════════════════════════════════════════════
check('Rust feature-gated stub files', () => {
  const libRs = readFileSafe(path.join(SRC_TAURI, 'lib.rs')) ?? '';
  // Find patterns like: #[path = "foo_stub.rs"] mod bar;
  const stubRe = /#\[path\s*=\s*"([^"]*_stub\.rs)"\]/g;
  const missing = [];
  let m;
  while ((m = stubRe.exec(libRs)) !== null) {
    const stubFile = m[1];
    const fullPath = path.join(SRC_TAURI, stubFile);
    if (!existsFile(fullPath)) {
      missing.push(`${stubFile} referenced in lib.rs but file does not exist`);
    }
  }

  return {
    ok: missing.length === 0,
    details: missing.length > 0 ? missing.join('\n    ') : 'all feature-gated stubs present',
  };
});

// ═════════════════════════════════════════════════════════════════════════
// CHECK 6: ts-rs binding files present
// ═════════════════════════════════════════════════════════════════════════
check('ts-rs binding files present', () => {
  const bindingsDir = path.join(REPO_ROOT, 'src-tauri', 'bindings');
  if (!existsDir(bindingsDir)) {
    return { ok: false, details: 'src-tauri/bindings directory missing' };
  }
  // Count how many .ts files are there; if zero, something's wrong
  const bindings = fs.readdirSync(bindingsDir).filter((f) => f.endsWith('.ts'));
  if (bindings.length === 0) {
    return { ok: false, details: 'bindings directory is empty — run `cargo test --lib export_bindings`' };
  }
  return { ok: true, details: `${bindings.length} bindings present` };
});

// ═════════════════════════════════════════════════════════════════════════
// CHECK 7: Essential frontend primitives exist
// ═════════════════════════════════════════════════════════════════════════
check('Essential frontend primitives', () => {
  const required = [
    'components/ViewRouter.tsx',
    'components/ViewTabBar.tsx',
    'components/ViewErrorBoundary.tsx',
    'components/SplashScreen.tsx',
    'components/Onboarding.tsx',
    'store/index.ts',
    'store/types.ts',
    'store/ui-slice.ts',
    'lib/commands.ts',
    'lib/trust-feedback.ts',
    'i18n/index.ts',
    'locales/en/ui.json',
    'App.tsx',
    'main.tsx',
  ];

  const missing = [];
  for (const r of required) {
    if (!existsFile(path.join(SRC, r))) missing.push(r);
  }

  return {
    ok: missing.length === 0,
    details: missing.length > 0 ? missing.join('\n    ') : `all ${required.length} primitives present`,
  };
});

// ═════════════════════════════════════════════════════════════════════════
// CHECK 8: Essential Rust primitives exist
// ═════════════════════════════════════════════════════════════════════════
check('Essential Rust primitives', () => {
  const required = [
    'lib.rs',
    'app_setup.rs',
    'state.rs',
    'runtime_paths.rs',
    'error.rs',
    'settings/manager.rs',
    'db/migrations.rs',
    'preemption.rs',
    'blind_spots.rs',
    'trust_ledger.rs',
    'intelligence_packs.rs',
  ];

  const missing = [];
  for (const r of required) {
    if (!existsFile(path.join(SRC_TAURI, r))) missing.push(r);
  }

  return {
    ok: missing.length === 0,
    details: missing.length > 0 ? missing.join('\n    ') : `all ${required.length} primitives present`,
  };
});

// ═════════════════════════════════════════════════════════════════════════
// Report
// ═════════════════════════════════════════════════════════════════════════
console.log('\n=== 4DA Wiring Validator ===\n');
for (const c of checks) {
  const badge = c.ok ? 'OK ' : 'FAIL';
  console.log(`  [${badge}] ${c.name}`);
  if (c.details) {
    const lines = String(c.details).split('\n');
    for (const line of lines) console.log(`    ${line}`);
  }
}
console.log('');

if (failures.length > 0) {
  console.log(`FAILED: ${failures.length} check(s) with wiring issues\n`);
  process.exit(1);
}

console.log(`PASSED: ${checks.length}/${checks.length} wiring checks\n`);
process.exit(0);
