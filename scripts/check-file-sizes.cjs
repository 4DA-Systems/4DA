/**
 * File Size Guardian — enforces line-count limits on source files.
 *
 * Usage:
 *   node scripts/check-file-sizes.cjs        # local check
 *   node scripts/check-file-sizes.cjs --ci   # CI mode (GitHub Actions annotations)
 */

'use strict';

const fs = require('fs');
const path = require('path');

// ============================================================================
// Configuration
// ============================================================================

const THRESHOLDS = {
  ts:  { warn: 300, error: 500 },
  tsx: { warn: 300, error: 450 },
  rs:  { warn: 500, error: 800 },
};

// Files that legitimately exceed thresholds — each must have a justification.
const EXCEPTIONS = {
  // Rust — files that exceed 800-line error threshold (justified or tracked as DEBT)
  'src-tauri/src/db/migrations.rs':        'DB schema migrations — sequential by nature',
  'src-tauri/src/db/sources.rs':           'Source item DB layer — CRUD + batch upsert + feed_health circuit breaker (6 methods + 5 tests)',
  'src-tauri/src/settings/mod.rs':         'Settings management — serialization + validation',
  'src-tauri/src/lib.rs':                  'App entrypoint — Tauri plugin registration',
  'src-tauri/src/sources/adapter_resilience_tests.rs': 'Resilience tests — 146 edge-case tests across all 10 source adapters',
  'src-tauri/src/scoring/pipeline_v2.rs':              'V2 scoring pipeline — 8-phase architecture with all signal extraction and combination',
  'src-tauri/src/scoring/context.rs':                  'Scoring context builder + 47 production-grade ACE synthesis tests (27 tests are production-readiness proofs)',
  'src-tauri/src/sso.rs':                              'SSO module — enterprise feature-gated, OAuth/SAML auth + 33 tests',
  'src-tauri/src/app_setup.rs':                        'App setup — Tauri setup() callback + pre-Tauri init + shutdown, all linear startup sequence',
  'src-tauri/src/monitoring_briefing.rs':               'Briefing orchestration — enrichment + diversity slots + novelty + morning schedule + 22 tests',
  'src-tauri/src/ace/scanner.rs':                       'Project scanner — 5 manifest parsers + 4 lockfile parsers (Cargo.lock, package-lock.json, pnpm-lock.yaml, yarn.lock) + relevance scoring + tests',
  'src-tauri/src/blind_spots.rs':                      'TEMP: slated for collapse into EvidenceItem + materializer in Intelligence Reconciliation Phase 4 (docs/strategy/INTELLIGENCE-RECONCILIATION.md). Remove this exception when phase ships.',
  'src-tauri/src/preemption.rs':                       'TEMP: grown with 50-entry suppression list + EvidenceItem conversion in Intelligence Reconciliation. Split suppression into its own module post-launch.',
  'src/components/preemption/PreemptionView.tsx':      'TEMP: grown with dismiss persistence (localStorage) + undo bar + debounce guard in Phase 13 polish. Extract dismiss logic into a hook post-launch.',

  // Rust — legacy debt (exposed when guardian threshold tightened from 1500→800 to match CLAUDE.md)
  // Each file is a candidate for splitting. Gate prevents new files from exceeding 800.
  'src-tauri/src/scoring/simulation/corpus.rs':        'DEBT: test corpus data — 1467 lines of fixture definitions',
  'src-tauri/src/signals.rs':                          'DEBT: signal chain detection + tests — split signal types from detection logic',
  'src-tauri/src/scoring/pipeline_tests.rs':           'DEBT: 1435 lines of pipeline tests — consider splitting by pipeline phase',
  'src-tauri/src/scoring/benchmark.rs':                'DEBT: scoring benchmarks + corpus — split corpus from benchmark harness',
  'src-tauri/src/llm.rs':                              'DEBT: LLM integration — provider logic + prompt construction + response parsing',
  'src-tauri/src/knowledge_decay.rs':                  'DEBT: knowledge decay detection + tests',
  'src-tauri/src/data_export.rs':                      'DEBT: multi-format export (JSON/CSV/PDF) — split by format',
  'src-tauri/src/scoring/dependencies.rs':             'DEBT: dependency scoring — extract lockfile matching from scoring logic',
  'src-tauri/src/ace/mod.rs':                          'DEBT: ACE module root — watcher + topic + embedding orchestration',
  'src-tauri/src/webhooks.rs':                         'DEBT: enterprise webhook system — feature-gated',
  'src-tauri/src/ace/behavior.rs':                     'DEBT: behavior analysis — pattern detection + tests',
  'src-tauri/src/achievement_engine.rs':               'DEBT: achievement system — feature-gated experimental',
  'src-tauri/src/ace/watcher.rs':                      'DEBT: filesystem watcher — event handling + debounce + tests',
  'src-tauri/src/calibration_fitter.rs':               'DEBT: curve fitting algorithm + tests',
  'src-tauri/src/streets_commands.rs':                 'DEBT: STREETS playbook commands — 20+ Tauri handlers',
  'src-tauri/src/developer_dna.rs':                    'DEBT: developer DNA profiling + SVG generation',
  'src-tauri/src/monitoring.rs':                       'DEBT: background monitoring orchestration',
  'src-tauri/src/signal_terminal.rs':                  'DEBT: signal terminal processing + tests',
  'src-tauri/src/settings/types.rs':                   'DEBT: Settings struct (33 fields) + sub-config types + defaults',
  'src-tauri/src/domain_profile.rs':                   'DEBT: domain profiling — extract data tables from logic',
  'src-tauri/src/team_sync_commands.rs':               'DEBT: team sync — feature-gated, 17 commands',
  'src-tauri/src/settings/license.rs':                 'DEBT: license validation + Keygen integration + tests',
  'src-tauri/src/source_fetching/mod.rs':              'DEBT: source fetching orchestration — scheduler + rate limiting',
  'src-tauri/src/startup_health.rs':                   'DEBT: startup diagnostics — Windows FFI + health checks',
  'src-tauri/src/source_fetching/fetcher.rs':          'DEBT: fetcher implementation — retry logic + circuit breaker',
  'src-tauri/src/signal_chains.rs':                    'DEBT: signal chain construction + tests',
  'src-tauri/src/settings_commands_license.rs':        'DEBT: license Tauri commands — validation + trial + tier logic',
  'src-tauri/src/source_config.rs':                    'DEBT: source configuration — 20+ source default configs',
  'src-tauri/src/health.rs':                           'DEBT: health monitoring — system + source + DB health checks',
  'src-tauri/src/digest.rs':                           'DEBT: digest generation — weekly/daily summary construction',
  'src-tauri/src/scoring/pipeline.rs':                 'DEBT: V1 scoring pipeline — kept alongside V2 for validation',
  'src-tauri/src/state.rs':                            'DEBT: app state — DB init + registry + global state management',
  'src-tauri/src/sources/osv.rs':                      'DEBT: OSV/CVE adapter — vulnerability parsing + tests',
  'src-tauri/src/analysis_deep_scan.rs':               'DEBT: deep analysis scan — content extraction + reranking',
  'src-tauri/src/analysis_rerank.rs':                  'DEBT: reranking pipeline — LLM-based relevance scoring',
  'src-tauri/src/commands.rs':                         'DEBT: misc Tauri commands — grab bag of handlers',
  'src-tauri/src/ollama.rs':                           'DEBT: Ollama integration — model management + health checks',
  'src-tauri/src/scoring/keywords.rs':                 'DEBT: keyword extraction + matching — data tables + logic',
  'src-tauri/src/content_dna.rs':                      'DEBT: content DNA — type detection + sophistication scoring',
  'src-tauri/src/enterprise_analytics_tests.rs':       'DEBT: enterprise analytics tests — feature-gated test suite',
  'src-tauri/src/context_engine.rs':                   'DEBT: context engine — query construction + embedding',
  'src-tauri/src/content_translation.rs':              'DEBT: content translation — multi-provider + batch + cache',
  'src-tauri/src/briefing_groundedness.rs':            'DEBT: briefing groundedness — claim extraction + verification',
  'src-tauri/src/ace/context.rs':                      'DEBT: ACE context builder — project graph + relevance',
  'src-tauri/src/embeddings.rs':                       'DEBT: embedding pipeline — Ollama + batch + cache',
  'src-tauri/src/stacks/profile_data_b.rs':            'DEBT: stack profile data tables — framework/tool definitions',
  'src-tauri/src/decisions.rs':                        'DEBT: decision journal — CRUD + MCP integration',
  'src-tauri/src/scoring/necessity.rs':                'DEBT: necessity scoring — information need detection',

  // TypeScript — type registries and complex UI
  'src/lib/commands.ts':                             'IPC command registry — all typed Tauri commands (280 with feature-gated variants)',
  'src/components/ResultsView.tsx':                  'Results list + filters — primary view',
  'src/components/void-engine/VoidHeartbeat.tsx':    'Void visualizer — canvas + animation',
  'src/App.tsx':                                     'App shell + routing — single entry point',
  'src/components/SourceConfigPanel.tsx':             'Source config form — per-source fields',
  'src/components/IndexedDocumentsPanel.tsx':          'Document browser — list + detail view',
  'src/components/DecisionMemory.tsx':                 'Decision memory — CRUD interface for tech decisions',
  'src/components/TechRadar.tsx':                      'Tech radar — technology adoption visualization',
  'src/components/__tests__/smoke.test.tsx':            'Smoke tests — all component render coverage',
  'src/components/FirstRunTransition.test.tsx':          'First-run experience — 18 tests across 6 phases',
  'src/components/result-item/ScoreBreakdownDrawer.tsx': 'Score drawer — factor extraction + comparison + feedback',
  'src/components/ResultsView.test.tsx':                  'ResultsView tests — 17 accessibility and state tests + required mocks',
  'src/components/__tests__/ActionBar.test.tsx':           'ActionBar tests — 26 tests covering all UI states and interactions',
  'src/store/types.ts':                                    'Store type registry — combined slice interfaces and AppStore union type',
  'src/components/onboarding/TasteTestStep.test.tsx':      'TasteTest tests — 22 tests across all onboarding phases',
  'src/components/NaturalLanguageSearch.tsx':               'NLS UI — search interface + results + ghost preview',
  'src/components/NaturalLanguageSearch.test.tsx':          'NLS tests — 14 tests for search UI states and interactions',
  'src/components/playbook/PlaybookView.test.tsx':         'Playbook tests — 15 tests for module navigation and progress',
  'src/components/SettingsModal.test.tsx':                  'Settings tests — comprehensive modal and tab interaction tests',
  'src/types/i18n-resources.d.ts':                         'Auto-generated i18n type declarations — regenerated via pnpm i18n:types',
  'src/components/blindspots/BlindSpotsView.tsx':          'TEMP: grown with title-first UI, signal matching, and noise reduction in Intelligence Reconciliation. Extract signal grouping into a hook post-launch.',
};

const SCAN_DIRS = ['src', 'src-tauri/src'];
// Note: .js files are not in THRESHOLDS and thus not checked.
// src/lib/fourda-components/*.js are GAME compiler output — exempt by design.
const SKIP_DIRS = new Set(['node_modules', 'target', 'dist', '.git', '_future']);

// ============================================================================
// Implementation
// ============================================================================

const ciMode = process.argv.includes('--ci');

function getExtension(filePath) {
  const ext = path.extname(filePath).slice(1);
  return ext;
}

function countLines(filePath) {
  const content = fs.readFileSync(filePath, 'utf8');
  // Count newlines; empty file = 0 lines
  if (content.length === 0) return 0;
  let count = 0;
  for (let i = 0; i < content.length; i++) {
    if (content[i] === '\n') count++;
  }
  // If file doesn't end with newline, count the last line
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

  if (lines > threshold.error) {
    errors.push({ file: normalized, lines, limit: threshold.error, level: 'error' });
  } else if (lines > threshold.warn) {
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
