#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# 4DA Release Readiness Gate
# Comprehensive pre-release validation: tests, builds, sovereignty, versions
# ─────────────────────────────────────────────────────────────────────────────

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# ── Colors ───────────────────────────────────────────────────────────────────
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

pass() { echo -e "  ${GREEN}[PASS]${RESET} $1"; }
fail() { echo -e "  ${RED}[FAIL]${RESET} $1"; }
warn() { echo -e "  ${YELLOW}[WARN]${RESET} $1"; }
info() { echo -e "  ${CYAN}[INFO]${RESET} $1"; }
step() { echo -e "\n${BOLD}[$1/$TOTAL_STEPS] $2${RESET}"; }

# ── Timing helpers ───────────────────────────────────────────────────────────
step_start() { STEP_START=$(date +%s); }
step_elapsed() {
  local end=$(date +%s)
  local dur=$((end - STEP_START))
  echo -e "  ${DIM}(${dur}s)${RESET}"
}

# ── State tracking ───────────────────────────────────────────────────────────
TOTAL_STEPS=9
HARD_FAILS=0
WARNINGS=0
RUST_TEST_COUNT=0
FRONTEND_TEST_COUNT=0
SUMMARY_LINES=()

record_pass() { SUMMARY_LINES+=("${GREEN}[PASS]${RESET} $1"); }
record_fail() { SUMMARY_LINES+=("${RED}[FAIL]${RESET} $1"); HARD_FAILS=$((HARD_FAILS + 1)); }
record_warn() { SUMMARY_LINES+=("${YELLOW}[WARN]${RESET} $1"); WARNINGS=$((WARNINGS + 1)); }

# ── Version argument ─────────────────────────────────────────────────────────
OPS_STATE="$REPO_ROOT/.claude/wisdom/ops-state.json"

read_json_field() {
  local file="$1" field="$2"
  node -e "
    const fs = require('fs');
    try {
      const data = JSON.parse(fs.readFileSync('$file', 'utf8'));
      const keys = '$field'.split('.');
      let val = data;
      for (const k of keys) { val = val?.[k]; }
      if (val !== undefined && val !== null) process.stdout.write(String(val));
    } catch(e) {}
  " 2>/dev/null || true
}

write_json_field() {
  local file="$1" field="$2" value="$3"
  node -e "
    const fs = require('fs');
    try {
      const data = JSON.parse(fs.readFileSync('$file', 'utf8'));
      const keys = '$field'.split('.');
      let obj = data;
      for (let i = 0; i < keys.length - 1; i++) {
        if (!obj[keys[i]] || typeof obj[keys[i]] !== 'object') obj[keys[i]] = {};
        obj = obj[keys[i]];
      }
      obj[keys[keys.length - 1]] = $value;
      fs.writeFileSync('$file', JSON.stringify(data, null, 2) + '\n');
    } catch(e) { console.error('Failed to write JSON:', e.message); process.exit(1); }
  "
}

if [ $# -ge 1 ]; then
  VERSION="$1"
else
  VERSION=$(node -e "process.stdout.write(require('./package.json').version)" 2>/dev/null || echo "unknown")
fi

SCRIPT_START=$(date +%s)

echo ""
echo -e "${BOLD}=== 4DA Release Readiness Gate v${VERSION} ===${RESET}"
echo -e "${DIM}$(date '+%Y-%m-%d %H:%M:%S')${RESET}"

# ─────────────────────────────────────────────────────────────────────────────
# STEP 1: Clean git state
# ─────────────────────────────────────────────────────────────────────────────
step 1 "Clean git state"
step_start

DIRTY_FILES=$(git status --porcelain 2>/dev/null || true)
if [ -n "$DIRTY_FILES" ]; then
  fail "Working directory has uncommitted changes:"
  echo "$DIRTY_FILES" | head -20
  if [ "$(echo "$DIRTY_FILES" | wc -l)" -gt 20 ]; then
    echo "  ... and more"
  fi
  record_fail "Git state: uncommitted changes"
else
  pass "Working directory clean"
  record_pass "Git state"
fi

step_elapsed

# ─────────────────────────────────────────────────────────────────────────────
# STEP 2: Rust tests
# ─────────────────────────────────────────────────────────────────────────────
step 2 "Rust tests (cargo test --lib)"
step_start

RUST_OUTPUT=$(cd src-tauri && cargo test --lib 2>&1) || true
RUST_RESULT=$(echo "$RUST_OUTPUT" | grep -E "^test result:" | tail -1 || true)

if echo "$RUST_RESULT" | grep -q "ok\."; then
  RUST_TEST_COUNT=$(echo "$RUST_RESULT" | grep -oP '\d+ passed' | grep -oP '\d+' || echo "0")
  RUST_FAILED=$(echo "$RUST_RESULT" | grep -oP '\d+ failed' | grep -oP '\d+' || echo "0")
  if [ "${RUST_FAILED:-0}" -gt 0 ]; then
    fail "Rust tests: ${RUST_TEST_COUNT} passed, ${RUST_FAILED} failed"
    record_fail "Rust tests: ${RUST_FAILED} failures"
  else
    pass "Rust tests: ${RUST_TEST_COUNT} passed"
    record_pass "Rust tests: ${RUST_TEST_COUNT} passed"
  fi
elif echo "$RUST_RESULT" | grep -q "FAILED"; then
  RUST_TEST_COUNT=$(echo "$RUST_RESULT" | grep -oP '\d+ passed' | grep -oP '\d+' || echo "0")
  RUST_FAILED=$(echo "$RUST_RESULT" | grep -oP '\d+ failed' | grep -oP '\d+' || echo "0")
  fail "Rust tests: ${RUST_TEST_COUNT} passed, ${RUST_FAILED} failed"
  echo "$RUST_OUTPUT" | grep -E "^failures:" -A 50 | head -30 || true
  record_fail "Rust tests: ${RUST_FAILED} failures"
else
  fail "Rust tests: could not parse results"
  echo "$RUST_OUTPUT" | tail -10
  record_fail "Rust tests: unparseable output"
fi

step_elapsed

# ─────────────────────────────────────────────────────────────────────────────
# STEP 3: Frontend tests
# ─────────────────────────────────────────────────────────────────────────────
step 3 "Frontend tests (vitest)"
step_start

FRONTEND_OUTPUT=$(pnpm run test -- --run 2>&1) || true

# Vitest outputs "Tests  N passed" or "Tests  N passed | M failed"
FRONTEND_PASSED_LINE=$(echo "$FRONTEND_OUTPUT" | grep -oP 'Tests\s+\d+ passed' | head -1 || true)
FRONTEND_FAILED_LINE=$(echo "$FRONTEND_OUTPUT" | grep -oP '\d+ failed' | head -1 || true)

if [ -n "$FRONTEND_PASSED_LINE" ]; then
  FRONTEND_TEST_COUNT=$(echo "$FRONTEND_PASSED_LINE" | grep -oP '\d+' | head -1 || echo "0")
  FRONTEND_FAILED=$(echo "$FRONTEND_FAILED_LINE" | grep -oP '\d+' || echo "0")
  if [ "${FRONTEND_FAILED:-0}" -gt 0 ] && [ "${FRONTEND_FAILED}" != "0" ]; then
    fail "Frontend tests: ${FRONTEND_TEST_COUNT} passed, ${FRONTEND_FAILED} failed"
    record_fail "Frontend tests: ${FRONTEND_FAILED} failures"
  else
    pass "Frontend tests: ${FRONTEND_TEST_COUNT} passed"
    record_pass "Frontend tests: ${FRONTEND_TEST_COUNT} passed"
  fi
else
  # Check if vitest exited with error
  if echo "$FRONTEND_OUTPUT" | grep -qi "error\|fail"; then
    fail "Frontend tests: execution failed"
    echo "$FRONTEND_OUTPUT" | tail -15
    record_fail "Frontend tests: execution failed"
  else
    fail "Frontend tests: could not parse results"
    echo "$FRONTEND_OUTPUT" | tail -10
    record_fail "Frontend tests: unparseable output"
  fi
fi

step_elapsed

# ─────────────────────────────────────────────────────────────────────────────
# STEP 4: Validation suite
# ─────────────────────────────────────────────────────────────────────────────
step 4 "Validation suite (validate:all)"
step_start

VALIDATE_EXIT=0
VALIDATE_OUTPUT=$(pnpm run validate:all 2>&1) || VALIDATE_EXIT=$?

if [ "$VALIDATE_EXIT" -eq 0 ]; then
  pass "Validation suite passed"
  record_pass "Validation suite"
else
  fail "Validation suite failed (exit code $VALIDATE_EXIT)"
  echo "$VALIDATE_OUTPUT" | grep -iE "error|fail|warning" | tail -20 || true
  record_fail "Validation suite"
fi

step_elapsed

# ─────────────────────────────────────────────────────────────────────────────
# STEP 5: Sovereignty score
# ─────────────────────────────────────────────────────────────────────────────
step 5 "Sovereignty score check"
step_start

if [ -f "$OPS_STATE" ]; then
  SOV_SCORE=$(read_json_field "$OPS_STATE" "sovereignty.score")
  if [ -n "$SOV_SCORE" ]; then
    if [ "$SOV_SCORE" -ge 80 ] 2>/dev/null; then
      pass "Sovereignty score: ${SOV_SCORE}/100"
      record_pass "Sovereignty: ${SOV_SCORE}/100"
    else
      warn "Sovereignty score below threshold: ${SOV_SCORE}/100 (need >= 80)"
      record_warn "Sovereignty: ${SOV_SCORE}/100 (< 80)"
    fi
  else
    warn "Sovereignty score not found in ops-state.json"
    record_warn "Sovereignty: score field missing"
  fi
else
  warn "ops-state.json not found — sovereignty check skipped"
  record_warn "Sovereignty: ops-state.json missing"
fi

step_elapsed

# ─────────────────────────────────────────────────────────────────────────────
# STEP 6: Cadence check
# ─────────────────────────────────────────────────────────────────────────────
step 6 "Cadence freshness check"
step_start

if [ -f "$OPS_STATE" ]; then
  OVERDUE_CADENCES=0
  NOW_EPOCH=$(date +%s)

  for cadence_key in lastDaily lastWeekly lastMonthly; do
    CADENCE_VAL=$(read_json_field "$OPS_STATE" "cadence.${cadence_key}")

    if [ -z "$CADENCE_VAL" ] || [ "$CADENCE_VAL" = "null" ]; then
      warn "${cadence_key}: never run"
      OVERDUE_CADENCES=$((OVERDUE_CADENCES + 1))
      continue
    fi

    # Parse ISO timestamp — node handles this reliably cross-platform
    CADENCE_EPOCH=$(node -e "process.stdout.write(String(Math.floor(new Date('$CADENCE_VAL').getTime()/1000)))" 2>/dev/null || echo "0")

    case "$cadence_key" in
      lastDaily)   MAX_AGE=172800  ;; # 48 hours (generous)
      lastWeekly)  MAX_AGE=1209600 ;; # 14 days
      lastMonthly) MAX_AGE=5184000 ;; # 60 days
    esac

    AGE=$((NOW_EPOCH - CADENCE_EPOCH))
    if [ "$AGE" -gt "$MAX_AGE" ]; then
      AGE_DAYS=$((AGE / 86400))
      warn "${cadence_key}: overdue (${AGE_DAYS} days ago)"
      OVERDUE_CADENCES=$((OVERDUE_CADENCES + 1))
    else
      AGE_DAYS=$((AGE / 86400))
      info "${cadence_key}: ${AGE_DAYS} days ago"
    fi
  done

  if [ "$OVERDUE_CADENCES" -gt 0 ]; then
    record_warn "Cadences: ${OVERDUE_CADENCES} overdue"
  else
    record_pass "Cadences: all current"
  fi
else
  warn "ops-state.json not found — cadence check skipped"
  record_warn "Cadences: ops-state.json missing"
fi

step_elapsed

# ─────────────────────────────────────────────────────────────────────────────
# STEP 7: Version consistency
# ─────────────────────────────────────────────────────────────────────────────
step 7 "Version consistency"
step_start

CARGO_VER=$(grep '^version' src-tauri/Cargo.toml | head -1 | cut -d'"' -f2)
TAURI_VER=$(node -e "process.stdout.write(require('./src-tauri/tauri.conf.json').version)" 2>/dev/null || echo "unknown")
PKG_VER=$(node -e "process.stdout.write(require('./package.json').version)" 2>/dev/null || echo "unknown")

VERSION_MISMATCH=0
if [ "$CARGO_VER" != "$VERSION" ]; then
  fail "Cargo.toml version: $CARGO_VER (expected $VERSION)"
  VERSION_MISMATCH=1
fi
if [ "$TAURI_VER" != "$VERSION" ]; then
  fail "tauri.conf.json version: $TAURI_VER (expected $VERSION)"
  VERSION_MISMATCH=1
fi
if [ "$PKG_VER" != "$VERSION" ]; then
  fail "package.json version: $PKG_VER (expected $VERSION)"
  VERSION_MISMATCH=1
fi

if [ "$VERSION_MISMATCH" -eq 0 ]; then
  pass "All versions match: $VERSION"
  record_pass "Version consistency: $VERSION"
else
  info "  Cargo.toml:      $CARGO_VER"
  info "  tauri.conf.json: $TAURI_VER"
  info "  package.json:    $PKG_VER"
  record_fail "Version mismatch"
fi

step_elapsed

# ─────────────────────────────────────────────────────────────────────────────
# STEP 8: Build artifacts
# ─────────────────────────────────────────────────────────────────────────────
step 8 "Build artifacts (tauri build)"
step_start

BUILD_EXIT=0
BUILD_OUTPUT=$(pnpm run tauri build 2>&1) || BUILD_EXIT=$?

if [ "$BUILD_EXIT" -ne 0 ]; then
  fail "Build failed (exit code $BUILD_EXIT)"
  echo "$BUILD_OUTPUT" | grep -iE "error" | tail -10 || true
  record_fail "Build failed"
else
  # Look for NSIS installer
  INSTALLER=$(find src-tauri/target/release/bundle/nsis -name "*.exe" 2>/dev/null | head -1 || true)
  if [ -n "$INSTALLER" ] && [ -f "$INSTALLER" ]; then
    INSTALLER_SIZE=$(ls -lh "$INSTALLER" | awk '{print $5}')
    pass "Installer found: $(basename "$INSTALLER") ($INSTALLER_SIZE)"
    record_pass "Build: $(basename "$INSTALLER") ($INSTALLER_SIZE)"
  else
    # Check for MSI as fallback
    INSTALLER=$(find src-tauri/target/release/bundle -name "*.msi" -o -name "*.exe" 2>/dev/null | head -1 || true)
    if [ -n "$INSTALLER" ] && [ -f "$INSTALLER" ]; then
      INSTALLER_SIZE=$(ls -lh "$INSTALLER" | awk '{print $5}')
      pass "Installer found: $(basename "$INSTALLER") ($INSTALLER_SIZE)"
      record_pass "Build: $(basename "$INSTALLER") ($INSTALLER_SIZE)"
    else
      fail "No installer found in bundle directory"
      record_fail "Build: no installer produced"
    fi
  fi
fi

step_elapsed

# ─────────────────────────────────────────────────────────────────────────────
# STEP 9: Record test counts
# ─────────────────────────────────────────────────────────────────────────────
step 9 "Record test counts"
step_start

TOTAL_TESTS=$((RUST_TEST_COUNT + FRONTEND_TEST_COUNT))

if [ "$TOTAL_TESTS" -gt 0 ] && [ -f "$OPS_STATE" ]; then
  TODAY=$(date '+%Y-%m-%d')
  ENTRY="{\"date\":\"${TODAY}\",\"rust\":${RUST_TEST_COUNT},\"frontend\":${FRONTEND_TEST_COUNT},\"total\":${TOTAL_TESTS}}"

  node -e "
    const fs = require('fs');
    try {
      const data = JSON.parse(fs.readFileSync('$OPS_STATE', 'utf8'));
      if (!data.testCounts) data.testCounts = {};
      if (!Array.isArray(data.testCounts.history)) data.testCounts.history = [];

      const entry = $ENTRY;

      // Replace entry for today if exists, otherwise append
      const idx = data.testCounts.history.findIndex(h => h.date === entry.date);
      if (idx >= 0) {
        data.testCounts.history[idx] = entry;
      } else {
        data.testCounts.history.push(entry);
      }

      // Keep last 30 entries
      if (data.testCounts.history.length > 30) {
        data.testCounts.history = data.testCounts.history.slice(-30);
      }

      data.testCounts.latest = entry;

      fs.writeFileSync('$OPS_STATE', JSON.stringify(data, null, 2) + '\n');
      process.stdout.write('ok');
    } catch(e) {
      console.error('Failed to update ops-state.json:', e.message);
      process.exit(1);
    }
  " 2>/dev/null && {
    pass "Recorded: Rust ${RUST_TEST_COUNT} + Frontend ${FRONTEND_TEST_COUNT} = ${TOTAL_TESTS} total"
    record_pass "Test counts recorded (${TOTAL_TESTS} total)"
  } || {
    warn "Failed to record test counts to ops-state.json"
    record_warn "Test count recording failed"
  }
elif [ "$TOTAL_TESTS" -eq 0 ]; then
  warn "No test counts to record (both suites returned 0)"
  record_warn "Test counts: nothing to record"
elif [ ! -f "$OPS_STATE" ]; then
  warn "ops-state.json not found — cannot record test counts"
  record_warn "Test counts: ops-state.json missing"
fi

step_elapsed

# ─────────────────────────────────────────────────────────────────────────────
# FINAL SUMMARY
# ─────────────────────────────────────────────────────────────────────────────
SCRIPT_END=$(date +%s)
TOTAL_ELAPSED=$((SCRIPT_END - SCRIPT_START))
MINUTES=$((TOTAL_ELAPSED / 60))
SECONDS=$((TOTAL_ELAPSED % 60))

echo ""
echo -e "${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
echo -e "${BOLD}  Release Readiness Summary — v${VERSION}${RESET}"
echo -e "${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
echo ""

for line in "${SUMMARY_LINES[@]}"; do
  echo -e "  $line"
done

echo ""
echo -e "  ${DIM}Time elapsed: ${MINUTES}m ${SECONDS}s${RESET}"
echo -e "  ${DIM}Tests: Rust ${RUST_TEST_COUNT} + Frontend ${FRONTEND_TEST_COUNT} = ${TOTAL_TESTS}${RESET}"
echo ""

if [ "$HARD_FAILS" -gt 0 ]; then
  echo -e "  ${RED}${BOLD}RELEASE BLOCKED${RESET} — ${HARD_FAILS} hard failure(s), ${WARNINGS} warning(s)"
  echo ""
  echo -e "  ${DIM}Fix all ${RED}[FAIL]${RESET}${DIM} items before release. ${YELLOW}[WARN]${RESET}${DIM} items are advisory.${RESET}"
  echo ""
  exit 1
else
  if [ "$WARNINGS" -gt 0 ]; then
    echo -e "  ${GREEN}${BOLD}RELEASE READY${RESET} — ${WARNINGS} warning(s), no hard failures"
  else
    echo -e "  ${GREEN}${BOLD}RELEASE READY${RESET} — all gates passed"
  fi
  echo ""
  echo -e "  ${BOLD}Next steps:${RESET}"
  echo "    1. git tag v${VERSION}"
  echo "    2. git push origin v${VERSION}"
  echo "    3. gh release create v${VERSION} --title 'v${VERSION}' --notes-file docs/RELEASE-NOTES-v${VERSION}.md"
  echo "    4. Sign the NSIS installer with minisign"
  echo "    5. Upload signed artifacts to the release"
  echo ""
  exit 0
fi
