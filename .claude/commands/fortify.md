---
description: "Codebase fortification intelligence — live metrics, coverage gaps, security surface, architecture health, dependency freshness, and highest-leverage trajectory"
allowed-tools: ["Read", "Glob", "Grep", "Task", "Bash(cargo test:*)", "Bash(cargo check:*)", "Bash(cargo clippy:*)", "Bash(cargo outdated:*)", "Bash(find:*)", "Bash(wc:*)", "Bash(pnpm run test:*)", "Bash(pnpm run validate:*)", "Bash(pnpm outdated:*)", "Bash(gh run:*)", "Bash(gh api:*)", "Bash(git:*)", "Bash(cat:*)", "Bash(sort:*)", "Bash(head:*)", "Bash(tail:*)", "Bash(echo:*)", "Bash(ls:*)", "Bash(date:*)", "Bash(time:*)"]
argument-hint: "[--quick | --deep | --rust | --frontend | --ci | --security | --deps | --trajectory]"
---

<objective>
Generate a live, evidence-based fortification report for the 4DA codebase. Every number comes from running actual commands against the current state — nothing cached, nothing assumed. The report covers test coverage, code health, security surface, architecture integrity, dependency freshness, CI pipeline status, and produces a ranked trajectory of highest-leverage next moves.

This command never modifies source code or commits changes. It writes only to `.claude/fortify-snapshot.json` to enable trend tracking across runs.

Scope filter: $ARGUMENTS
- `--quick` — vital signs table only (30 seconds)
- `--deep` — full report + parallel subagent deep dives into each subsystem
- `--rust` — Rust backend analysis only
- `--frontend` — Frontend analysis only
- `--ci` — CI pipeline health only
- `--security` — security surface analysis only
- `--deps` — dependency health only
- `--trajectory` — still runs Phase 1 (vital signs) for data anchor, then skips Phases 2-5 and jumps to Phase 6 trajectory analysis
- Default (no flag): full report with trajectory (all sections)
</objective>

<context>
Pre-load at invocation time:
- Current commit: `! \`git rev-parse --short HEAD\``
- Current branch: `! \`git branch --show-current\``
- Working directory status: `! \`git status --porcelain | wc -l\``
- Last fortify snapshot (if exists): `! \`cat .claude/fortify-snapshot.json 2>/dev/null || echo "no previous snapshot"\``
</context>

<process>

## Phase 1: Vital Signs (always runs — even with --quick)

Gather these metrics in parallel. Each metric must produce a concrete number.

### 1A. Rust Unit Tests
```bash
cd src-tauri && cargo test --lib 2>&1 | tail -3
```
Extract: passed count, failed count, ignored count, duration.

### 1B. Rust Integration Tests (individually — never use --all)
```bash
cd src-tauri && for t in tests/*.rs; do name=$(basename "$t" .rs); result=$(cargo test --test "$name" 2>&1 | tail -1); echo "$name: $result"; done
```
Extract: per-file pass counts, total integration count, any compilation failures.

### 1C. Frontend Tests
```bash
cd "$(git rev-parse --show-toplevel)" && pnpm run test -- --run 2>&1 | tail -10
```
Extract: test file count, total test count, pass/fail, duration.

### 1D. Rust File Coverage
```bash
cd src-tauri && total=$(find src -name "*.rs" | wc -l) && tested=$(find src -name "*.rs" -exec grep -l '#\[cfg(test)\]\|#\[test\]' {} \; | wc -l) && echo "Total: $total Tested: $tested Untested: $((total - tested))"
```

### 1E. Frontend File Coverage
```bash
cd "$(git rev-parse --show-toplevel)" && test_files=$(find src -name "*.test.*" -o -name "*.spec.*" | wc -l) && components=$(find src/components -name "*.tsx" -not -name "*.test.*" -not -path "*__tests__*" 2>/dev/null | wc -l) && store_tests=$(find src/store/__tests__ -name "*.test.*" 2>/dev/null | wc -l) && store_total=$(find src/store -maxdepth 1 -name "*-slice.ts" 2>/dev/null | wc -l) && echo "Test files: $test_files Components: $components Store tested: $store_tests / $store_total"
```

### 1F. LOC Counts (separate commands for reliability)
```bash
cd src-tauri && find src -name "*.rs" -exec cat {} + | wc -l
```
```bash
cd "$(git rev-parse --show-toplevel)" && find src -name "*.ts" -o -name "*.tsx" | grep -v node_modules | grep -v ".test." | grep -v "__tests__" | xargs cat 2>/dev/null | wc -l
```

### 1G. CI Pipeline Status
```bash
gh run list --limit 3 --json status,conclusion,name,headBranch 2>/dev/null || echo "gh CLI not available — skip CI status"
```

### 1H. Build Health (quick sanity)
```bash
cd src-tauri && cargo check 2>&1 | tail -5
```

If `$ARGUMENTS` is `--quick`, skip to Phase 7 (Report) using only Phase 1 data.

---

## Phase 2: Coverage Deep Dive (skip if --quick or --ci or --security or --deps)

### 2A. Untested Rust Files — Ranked by Line Count
Use Grep to find all `.rs` files in `src-tauri/src/` that do NOT contain `#[cfg(test)]` or `#[test]`. For each, count lines. Sort descending. Show top 25.

### 2B. Test Density Per Tested File
For files that DO have tests, count the number of `#[test]` annotations. Classify:
- **Locked down** (10+ tests): list files
- **Well tested** (5-9 tests): list files
- **Lightly tested** (1-4 tests): list files — these are expansion candidates

### 2C. Untested Frontend Components — Ranked by Line Count
Find all `.tsx` files in `src/components/` that have no corresponding `.test.tsx` or entry in `__tests__/`. Sort by line count descending. Show top 20.

### 2D. Store Slice Coverage
For each `*-slice.ts` file in `src/store/`, check if a matching test file exists in `src/store/__tests__/`. Report tested vs untested.

### 2E. IPC Boundary Audit
This is the critical bridge between frontend and backend. Analyze:

1. Count total registered commands in `src-tauri/src/lib.rs` (inside `generate_handler![]`)
2. For each `*_commands.rs` file, check if it has `#[cfg(test)]` module
3. Report: N command files, N with tests, N without tests
4. List the untested command files with their line counts

The 17 known command files:
- `ace_commands.rs`, `autophagy_commands.rs`, `content_commands.rs`, `context_commands.rs`
- `decision_advantage_commands.rs`, `digest_commands.rs`, `game_commands.rs`, `health_commands.rs`
- `monitoring_commands.rs`, `playbook_commands.rs`, `settings_commands.rs`, `stack_commands.rs`
- `streets_commands.rs`, `suns_commands.rs`, `tech_radar_commands.rs`, `translation_commands.rs`
- `void_commands.rs`

---

## Phase 3: Critical Systems Matrix (skip if --quick or --ci or --deps)

For each critical system, determine test status AND test quality. Use Grep on each file.

| # | System | File(s) | Check |
|---|--------|---------|-------|
| 1 | Database migrations | `src/db/migrations.rs` | `#[cfg(test)]` present? How many `#[test]` fns? |
| 2 | Database sources | `src/db/sources.rs` | Same |
| 3 | Database cache | `src/db/cache.rs` | Same |
| 4 | Database history | `src/db/history.rs` | Same |
| 5 | Scoring pipeline | `src/scoring/pipeline.rs` | Same |
| 6 | Scoring semantic | `src/scoring/semantic.rs` | Same |
| 7 | Scoring dedup | `src/scoring/dedup.rs` | Same |
| 8 | Scoring calibration | `src/scoring/calibration.rs` | Same |
| 9 | Analysis orchestrator | `src/analysis.rs` | Same |
| 10 | Embeddings pipeline | `src/embeddings.rs` | Same |
| 11 | Source fetcher | `src/source_fetching/fetcher.rs` | Same |
| 12 | ACE behavior | `src/ace/behavior.rs` | Same |
| 13 | ACE context | `src/ace/context.rs` | Same |
| 14 | ACE topic embeddings | `src/ace/topic_embeddings.rs` | Same |
| 15 | Signals engine | `src/signals.rs` | Same |
| 16 | Utils (shared) | `src/utils.rs` | Same |
| 17 | Achievement engine | `src/achievement_engine.rs` | Same |
| 18 | Events | `src/events.rs` | Same |

Report: `N/18 critical systems tested` with per-system status and test count.

---

## Phase 4: Security Surface Scan (skip if --quick or --ci or --deps or --frontend)

### 4A. Unsafe Code & Panicking Paths
```bash
cd src-tauri && grep -rn "unsafe\s" src/ --include="*.rs" | grep -v "// unsafe" | grep -v test | wc -l
```
```bash
cd src-tauri && grep -rn "\.unwrap()" src/ --include="*.rs" | grep -v test | grep -v "// " | wc -l
```
```bash
cd src-tauri && grep -rn "panic!\|todo!\|unimplemented!" src/ --include="*.rs" | grep -v test | wc -l
```

### 4B. Blanket Suppressions
Search for file-level `#![allow(dead_code)]`, `#![allow(unused)]`, or `#![allow(clippy::` that suppress entire categories of warnings. These hide real issues.
```bash
cd src-tauri && grep -rn '#!\[allow(' src/ --include="*.rs" | grep -v test
```
Report each occurrence with file and line.

### 4C. Clippy Health
```bash
cd src-tauri && cargo clippy --lib 2>&1 | grep -c "warning\[" || echo "0 warnings"
```

### 4D. SQL Injection Surface
Check for string-formatted SQL (interpolated queries without parameterized binding):
Use Grep to search for patterns like `format!("SELECT`, `format!("INSERT`, `format!("UPDATE`, `format!("DELETE` in non-test Rust code. Each match is a potential injection vector.

### 4E. Secret Exposure Risk
Check for hardcoded API keys, tokens, or credentials:
Use Grep to search for patterns like `api_key = "`, `token = "`, `secret = "`, `password = "` in source files (excluding tests and examples).

Report: Security surface score (lower is better) = unwrap_count + panic_count + blanket_suppression_count + sql_format_count + secret_count.

---

## Phase 5: Dependency Health (skip if --quick or --ci or --security)

### 5A. Rust Dependencies
```bash
cd src-tauri && cargo outdated --root-deps-only 2>/dev/null || echo "cargo-outdated not installed — skip"
```
If cargo-outdated isn't available, read `Cargo.toml` and list the major dependencies with their versions.

### 5B. Frontend Dependencies
```bash
cd "$(git rev-parse --show-toplevel)" && pnpm outdated 2>/dev/null | head -30 || echo "pnpm outdated unavailable"
```

### 5C. File Size Compliance
```bash
cd "$(git rev-parse --show-toplevel)" && pnpm run validate:sizes 2>&1 | tail -20
```
Report any files exceeding the limits (TypeScript: 500 lines, Rust: 1000 lines).

### 5D. Build Performance
```bash
cd src-tauri && time cargo check 2>&1 | tail -3
```
Report: cargo check time (incremental), note if this is getting slower.

---

## Phase 6: Trajectory Analysis (skip if --quick or --ci)

Based on ALL data gathered in Phases 1-5, identify the **7 highest-leverage moves** using this scoring framework:

### Priority Scoring (each factor 1-5, total out of 25)

1. **Blast radius** (1-5): How many other systems depend on this untested/unhealthy code?
   - 5 = everything depends on it (db, scoring pipeline)
   - 1 = standalone module with no dependents
2. **Risk surface** (1-5): Line count x complexity of untested code
   - 5 = 500+ lines, complex logic
   - 1 = <100 lines, simple delegation
3. **User-facing impact** (1-5): Does this code directly affect what users see?
   - 5 = renders the main view, scores results
   - 1 = internal plumbing, background tasks
4. **Testability** (1-5): Can this be tested with existing infrastructure?
   - 5 = pure functions, uses test_utils.rs/factories.ts directly
   - 1 = requires new mocks, HTTP stubs, or complex setup
5. **Compound value** (1-5): Does fixing this unlock testing/fixing other things?
   - 5 = testing this makes 10 other files testable
   - 1 = isolated improvement, no cascade

### Output Format Per Move

```
### Move N: [Specific Action Verb] [Specific Target]
**Score:** N/25 (blast: N, risk: N, user: N, testability: N, compound: N)
**File:** [path] — [N] lines, [N] existing tests
**Why now:** [2-3 sentences: what evidence from this report makes this the right move]
**What breaks if wrong:** [concrete failure scenario]
**Testing strategy:**
  1. [Specific test to write — name, what it asserts]
  2. [Second test]
  3. [Third test]
**Estimated yield:** [N] new tests
**Dependencies:** [any test infrastructure needed first, or "none — start immediately"]
```

After the 7 moves, add:

### Moves NOT Recommended Right Now (and why)
List 3 things that might seem important but are actually low-leverage given current state. Explain why to prevent wasted effort.

---

## Phase 7: Report Assembly

Produce the final report in this exact structure:

```markdown
# 4DA Fortification Report
**Date:** [from context] | **Commit:** [from context] | **Branch:** [from context]
**Uncommitted changes:** [from context]

---

## Vital Signs

| Metric | Current | Target | Status |
|--------|---------|--------|--------|
| Rust unit tests | N | 1,000 | [bar: N/1000] |
| Rust integration tests | N | 200 | [bar] |
| Frontend tests | N | 500 | [bar] |
| **Total tests** | **N** | **1,700** | |
| Rust files tested | N/N (N%) | 75% | |
| Frontend test files | N | — | |
| Store slices tested | N/N (N%) | 75% | |
| Critical systems | N/18 | 18/18 | |
| IPC command files tested | N/17 | 17/17 | |
| Rust LOC | N | — | |
| Frontend LOC | N | — | |

## Test Quality Distribution (Rust)

| Category | Count | Files |
|----------|-------|-------|
| Locked down (10+ tests) | N | [list] |
| Well tested (5-9) | N | [list] |
| Lightly tested (1-4) | N | [list] |
| Zero tests | N | [top 15 by size] |

## IPC Boundary Status

| Command File | Lines | Tests | Status |
|-------------|-------|-------|--------|
| [each of 17 files] | N | N | tested/untested |

## Critical Systems Matrix

| # | System | File | Lines | Tests | Status |
|---|--------|------|-------|-------|--------|
| [each of 18 systems] | | | | | |

## Security Surface

| Metric | Count | Trend |
|--------|-------|-------|
| Production unwrap() calls | N | |
| panic!/todo!/unimplemented! | N | |
| Blanket #![allow(..)] | N | |
| String-formatted SQL | N | |
| Clippy warnings | N | |
| **Security score** | **N** | **(lower is better)** |

## Dependency Health

| Area | Status | Details |
|------|--------|---------|
| Rust outdated | N packages | [summary] |
| Frontend outdated | N packages | [summary] |
| File size violations | N files | [list if any] |
| Build time (cargo check) | Ns | |

## Coverage Heat Map

### Backend — Fully Tested
[files with 5+ tests]

### Backend — Needs Expansion
[files with 1-4 tests]

### Backend — No Coverage (top 15 by LOC)
[untested files sorted by line count]

### Frontend — Tested
[components with test files]

### Frontend — No Coverage (top 15 by LOC)
[untested components sorted by line count]

### Store Slices
[tested vs untested]

## Progress Since Last Snapshot

| Metric | Previous | Current | Delta |
|--------|----------|---------|-------|
[If previous snapshot exists, show deltas. If not, note "First run — no comparison available."]

---

## Trajectory: 7 Highest-Leverage Moves

[Move 1-7 in the format from Phase 6]

## Anti-Trajectory: 3 Things to Skip

[Moves NOT recommended and why]

---

## Session Recommendation

> [One sentence: THE single most impactful action to take right now — specific file, specific approach, specific outcome]

**Yield:** [N] new tests | **Unlocks:** [what becomes possible after]
```

---

## Phase 8: Save Snapshot (for trend tracking)

After generating the report, save a JSON snapshot of the key metrics for comparison in future runs:

```bash
cat > .claude/fortify-snapshot.json << 'SNAPSHOT'
{
  "date": "[today]",
  "commit": "[SHA]",
  "rust_unit_tests": N,
  "rust_integration_tests": N,
  "frontend_tests": N,
  "total_tests": N,
  "rust_files_total": N,
  "rust_files_tested": N,
  "frontend_test_files": N,
  "store_slices_tested": N,
  "critical_systems_tested": N,
  "ipc_files_tested": N,
  "security_score": N,
  "clippy_warnings": N,
  "unwrap_count": N
}
SNAPSHOT
```

This enables the "Progress Since Last Snapshot" section in future runs.

</process>

<success_criteria>
The report is complete when ALL of these are true:
1. Every metric in the Vital Signs table has a concrete number (no "N/A" or "unknown" unless a tool genuinely failed, with the failure reason noted)
2. At least 15 untested files are listed with line counts in the coverage heat map
3. All 18 critical systems have a tested/untested determination
4. All 17 IPC command files have a tested/untested determination
5. The security surface score is computed from actual grep counts
6. All 7 trajectory moves have scores, specific files, and specific testing strategies
7. The session recommendation names a single specific file and action
8. If a previous snapshot exists, deltas are computed and displayed
9. A new snapshot JSON is written for future comparisons
10. No section says "write more tests" — every recommendation names the exact file and approach
</success_criteria>

<edge_cases>
- **Dev server running (exe locked):** Use `cargo test --lib` and `cargo check --lib` to avoid rebuilding the binary
- **`cargo test --all` compilation failures:** Never use `--all`. Phase 1B runs integration tests individually.
- **`cargo-outdated` not installed:** Skip Rust dependency freshness, note in report
- **`gh` CLI not available:** Skip CI status, note as unavailable
- **No previous snapshot:** Skip "Progress Since Last Snapshot" section, note "First run"
- **Database missing:** Not needed — all checks are static analysis and test execution
- **Ollama not running:** Not needed — no embedding operations performed
- **Partial flag (e.g., --rust):** Only run phases relevant to that scope. Still produce Phase 7 report format but with "N/A — out of scope" for skipped sections
</edge_cases>

<constraints>
- NEVER modify source code or commit changes — this command only writes to `.claude/fortify-snapshot.json`
- NEVER run `cargo test --all` (known compilation issue with stack_simulation in --all mode)
- NEVER run `pnpm run test -- --coverage` (slow, reserved for CI)
- NEVER run `cargo build` or `pnpm run tauri build` (too slow for a health check)
- NEVER compare against numbers you're guessing — only compare against the snapshot file or known targets
- NEVER suggest generic improvements ("improve coverage") — every recommendation must name the specific file, the specific function, and the specific test to write
- NEVER skip a phase without noting why (flag-based skip is fine, error-based skip must be documented)
- If `--deep` is passed, spawn parallel Task subagents for: (a) Rust coverage deep dive, (b) Frontend coverage deep dive, (c) Security surface scan, (d) Dependency health audit — then merge results
</constraints>
