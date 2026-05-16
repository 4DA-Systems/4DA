/**
 * File Size Guardian — enforces line-count limits on source files.
 *
 * Usage:
 *   node scripts/check-file-sizes.cjs        # local check
 *   node scripts/check-file-sizes.cjs --ci   # CI mode (GitHub Actions annotations)
 *
 * Limits calibrated against actual distribution (2026-05-10):
 *   Rust:  median 379, p90 870 → warn 700, error 1000
 *   TS:    median  82, p90 296 → warn 300, error 500
 *   TSX:   similar to TS        → warn 350, error 500
 *
 * Test files (*.test.*, *_tests.rs) are exempt from warnings
 * because thorough testing should not be penalized. They still
 * trigger errors at the hard limit.
 */

'use strict';

const fs = require('fs');
const path = require('path');

// ============================================================================
// Configuration
// ============================================================================

const THRESHOLDS = {
  ts:  { warn: 300, error: 500 },
  tsx: { warn: 350, error: 500 },
  rs:  { warn: 700, error: 1000 },
};

// Files that legitimately exceed the error threshold — each must have a justification.
const EXCEPTIONS = {
  // ── Rust — justified large files ──────────────────────────────────────────
  'src-tauri/src/db/migrations.rs':        'DB schema migrations — sequential by nature',
  'src-tauri/src/db/sources.rs':           'Source item DB layer — CRUD + batch upsert + circuit breaker',
  'src-tauri/src/lib.rs':                  'App entrypoint — Tauri plugin registration + command wiring',
  'src-tauri/src/sources/adapter_resilience_tests.rs': 'Resilience tests — 146 edge-case tests across all source adapters',
  'src-tauri/src/scoring/pipeline_v2.rs':  'V2 scoring pipeline — 8-phase architecture',
  'src-tauri/src/scoring/context.rs':      'Scoring context builder + 47 ACE synthesis tests',
  'src-tauri/src/sso.rs':                  'SSO module — enterprise feature-gated, OAuth/SAML + tests',
  'src-tauri/src/app_setup.rs':            'App setup — Tauri setup() callback + pre-init + shutdown',
  'src-tauri/src/monitoring_briefing.rs':  'Briefing orchestration — enrichment + slots + novelty + tests',
  'src-tauri/src/ace/scanner.rs':          'Project scanner — manifest + lockfile parsers + tests',
  'src-tauri/src/blind_spots.rs':          'TEMP: slated for collapse into EvidenceItem in Intelligence Reconciliation Phase 4',
  'src-tauri/src/preemption.rs':           'TEMP: split suppression list into own module post-launch',
  'src-tauri/src/trust_ledger.rs':         'Trust feedback + SQLite outbox commands — all trust-related',

  // Rust — files over error threshold, candidates for splitting
  'src-tauri/src/scoring/simulation/corpus.rs': 'Test corpus data — fixture definitions',
  'src-tauri/src/signals.rs':              'Signal chain detection + tests — split signal types from detection',
  'src-tauri/src/scoring/pipeline_tests.rs': 'Pipeline tests — consider splitting by phase',
  'src-tauri/src/scoring/benchmark.rs':    'Scoring benchmarks + corpus — split corpus from harness',
  'src-tauri/src/llm.rs':                  'LLM integration — provider logic + prompts + response parsing',
  'src-tauri/src/knowledge_decay.rs':      'Knowledge decay detection + tests',
  'src-tauri/src/data_export.rs':          'Multi-format export (JSON/CSV/PDF) — split by format',
  'src-tauri/src/scoring/dependencies.rs': 'Dependency scoring — extract lockfile matching from scoring',
  'src-tauri/src/ace/mod.rs':              'ACE module root — orchestration',
  'src-tauri/src/webhooks.rs':             'Enterprise webhook system — feature-gated',
  'src-tauri/src/ace/behavior.rs':         'Behavior analysis — pattern detection + tests',
  'src-tauri/src/achievement_engine.rs':   'Achievement system — feature-gated experimental',
  'src-tauri/src/ace/watcher.rs':          'Filesystem watcher — event handling + debounce + tests',
  'src-tauri/src/calibration_fitter.rs':   'Curve fitting algorithm + tests',
  'src-tauri/src/streets_commands.rs':     'STREETS playbook commands — 20+ Tauri handlers',
  'src-tauri/src/monitoring.rs':           'Background monitoring orchestration',
  'src-tauri/src/signal_terminal.rs':      'Signal terminal processing + tests',
  'src-tauri/src/settings/types.rs':       'Settings struct (33 fields) + sub-config types + defaults',
  'src-tauri/src/domain_profile.rs':       'Domain profiling — extract data tables from logic',
  'src-tauri/src/team_sync_commands.rs':   'Team sync — feature-gated, 17 commands',
  'src-tauri/src/settings/license.rs':     'License validation + Keygen integration + tests',
  'src-tauri/src/source_fetching/mod.rs':  'Source fetching orchestration — scheduler + rate limiting',
  'src-tauri/src/bin/cli.rs':              'Standalone CLI binary — cannot split (cli/ gitignored for workspace)',

  // ── TypeScript — justified large files ────────────────────────────────────
  'src/lib/commands.ts':                   'IPC command registry — all typed Tauri commands',
  'src/store/slice-types.ts':               'Store slice interfaces — 16 pure type definitions',
  'src/types/i18n-resources.d.ts':         'Auto-generated i18n type declarations — regenerated via pnpm i18n:types',
  'src/components/playbook/StreetsCodeBlock.tsx': 'Code block with syntax highlighting — 2 lines over, no clean split point',
  'src/components/enterprise/SsoConfigPanel.tsx': 'SSO config form — enterprise feature-gated, 5 lines over',
  'src/components/IntelligenceReport.tsx':  'Intelligence report card — single component, 9 lines over',
  'src/components/preemption/PreemptionCard.tsx': 'Preemption card — single component, 9 lines over',
};

const SCAN_DIRS = ['src', 'src-tauri/src'];
const SKIP_DIRS = new Set(['node_modules', 'target', 'dist', '.git', '_future']);

// ============================================================================
// Implementation
// ============================================================================

const ciMode = process.argv.includes('--ci');

function isTestFile(filePath) {
  const name = path.basename(filePath);
  return name.includes('.test.') || name.includes('.spec.') || name.endsWith('_tests.rs');
}

function getExtension(filePath) {
  return path.extname(filePath).slice(1);
}

function countLines(filePath) {
  const content = fs.readFileSync(filePath, 'utf8');
  if (content.length === 0) return 0;
  let count = 0;
  for (let i = 0; i < content.length; i++) {
    if (content[i] === '\n') count++;
  }
  if (content[content.length - 1] !== '\n') count++;
  return count;
}

function walkDir(dir, results) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }

  for (const entry of entries) {
    if (SKIP_DIRS.has(entry.name)) continue;

    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walkDir(fullPath, results);
    } else if (entry.isFile()) {
      const ext = getExtension(entry.name);
      if (THRESHOLDS[ext]) {
        results.push(fullPath);
      }
    }
  }
}

function normalizeSlashes(p) {
  return p.replace(/\\/g, '/');
}

// Collect all source files
const files = [];
for (const dir of SCAN_DIRS) {
  if (fs.existsSync(dir)) {
    walkDir(dir, files);
  }
}

const warnings = [];
const errors = [];

for (const filePath of files) {
  const normalized = normalizeSlashes(filePath);
  if (EXCEPTIONS[normalized]) continue;

  const ext = getExtension(filePath);
  const threshold = THRESHOLDS[ext];
  if (!threshold) continue;

  const lines = countLines(filePath);

  const testFile = isTestFile(filePath);
  const errorLimit = testFile ? threshold.error * 2 : threshold.error;

  if (lines > errorLimit) {
    errors.push({ file: normalized, lines, limit: errorLimit, level: 'error' });
  } else if (lines > threshold.warn && !testFile) {
    warnings.push({ file: normalized, lines, limit: threshold.warn, level: 'warn' });
  }
}

// ============================================================================
// Output
// ============================================================================

const allIssues = [...errors, ...warnings].sort((a, b) => b.lines - a.lines);

if (allIssues.length === 0) {
  console.log('File size check passed — all files within limits.');
  process.exit(0);
}

// Table output
const maxFileLen = Math.max(...allIssues.map(i => i.file.length), 4);
const header = 'File'.padEnd(maxFileLen) + '  Lines  Limit  Status';
console.log('\n' + header);
console.log('-'.repeat(header.length));

for (const issue of allIssues) {
  const status = issue.level === 'error' ? 'ERROR' : 'warn';
  const line = `${issue.file.padEnd(maxFileLen)}  ${String(issue.lines).padStart(5)}  ${String(issue.limit).padStart(5)}  ${status}`;
  console.log(line);

  if (ciMode) {
    const annotation = issue.level === 'error' ? '::error' : '::warning';
    console.log(`${annotation} file=${issue.file}::File has ${issue.lines} lines (limit: ${issue.limit}). Split or add to exception list.`);
  }
}

console.log('');

if (errors.length > 0) {
  console.log(`${errors.length} file(s) exceed error threshold. Split large files or add justified exceptions.`);
  process.exit(1);
} else {
  console.log(`${warnings.length} file(s) approaching size limits (warnings only).`);
  process.exit(0);
}
