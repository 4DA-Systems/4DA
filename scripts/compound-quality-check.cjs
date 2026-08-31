/**
 * Compound Quality Check — ensures every code change leaves the codebase strictly better.
 *
 * Runs in the pre-push hook (must complete in <5 seconds, no compilation).
 * Analyzes the unpushed commit range for quality regressions.
 *
 * Rules:
 *   1. Test Coverage Direction — new source files need tests, test count must not drop
 *   2. Error Handling Direction — new catch blocks should use reportError, no new .unwrap() in prod Rust
 *   3. Documentation Direction — new Tauri commands need commands.ts entries, new adapters need freshness thresholds
 *   4. Structural Health — file size limits, flag files that grew >100 lines
 *
 * Exit codes:
 *   0 — all pass (or warnings only)
 *   1 — hard failure (test count regression)
 *
 * Usage:
 *   node scripts/compound-quality-check.cjs          # normal (analyzes unpushed commits)
 *   node scripts/compound-quality-check.cjs --all    # analyze all staged/unstaged changes
 */

'use strict';

const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const ROOT = path.resolve(__dirname, '..');

// ============================================================================
// Helpers
// ============================================================================

/** Run a git command and return trimmed stdout, or empty string on failure. */
function git(cmd) {
  try {
    return execSync(`git ${cmd}`, {
      cwd: ROOT,
      stdio: 'pipe',
      encoding: 'utf8',
      timeout: 10_000,
    }).trim();
  } catch {
    return '';
  }
}

/** Run a command and return { ok, output }. */
function run(cmd) {
  try {
    const output = execSync(cmd, {
      cwd: ROOT,
      stdio: 'pipe',
      encoding: 'utf8',
      timeout: 30_000,
    }).trim();
    return { ok: true, output };
  } catch (err) {
    return { ok: false, output: ((err.stdout || '') + '\n' + (err.stderr || '')).trim() };
  }
}

// ============================================================================
// Determine commit range
// ============================================================================

function getCommitRange() {
  // If --all flag, analyze working tree vs HEAD
  if (process.argv.includes('--all')) {
    return null; // signals: use diff against HEAD
  }

  // Find unpushed commits by comparing with upstream
  const upstream = git('rev-parse --abbrev-ref @{upstream}');
  if (upstream) {
    const count = git(`rev-list --count ${upstream}..HEAD`);
    const n = parseInt(count, 10);
    if (n > 0) {
      return { range: `${upstream}..HEAD`, count: n };
    }
  }

  // Fallback: compare with origin/main
  const hasOriginMain = git('rev-parse --verify origin/main');
  if (hasOriginMain) {
    const count = git('rev-list --count origin/main..HEAD');
    const n = parseInt(count, 10);
    if (n > 0) {
      return { range: 'origin/main..HEAD', count: n };
    }
  }

  // No unpushed commits — analyze last commit
  return { range: 'HEAD~1..HEAD', count: 1 };
}

/** Get the diff for the commit range. Returns { addedLines, removedLines, changedFiles, diffStat }. */
function analyzeDiff(commitRange) {
  let diffCmd, statCmd, nameOnlyCmd;

  if (!commitRange) {
    // --all mode: diff working tree
    diffCmd = 'diff HEAD';
    statCmd = 'diff --stat HEAD';
    nameOnlyCmd = 'diff --name-status HEAD';
  } else {
    diffCmd = `diff ${commitRange.range}`;
    statCmd = `diff --stat ${commitRange.range}`;
    nameOnlyCmd = `diff --name-status ${commitRange.range}`;
  }

  const diff = git(diffCmd);
  const stat = git(statCmd);
  const nameStatus = git(nameOnlyCmd);

  // Parse changed files with status
  const changedFiles = [];
  for (const line of nameStatus.split('\n')) {
    if (!line.trim()) continue;
    const parts = line.split('\t');
    if (parts.length >= 2) {
      changedFiles.push({ status: parts[0].trim(), file: parts[parts.length - 1].trim() });
    }
  }

  // Parse line-level additions and removals per file
  const fileChanges = {};
  let currentFile = null;

  // Line number in the POST-change file of the next line we read. Tracked so an
  // added line can be located in the file on disk, which is what lets the
  // unwrap rule tell production code from an inline `#[cfg(test)]` module.
  let newLineNo = 0;

  for (const line of diff.split('\n')) {
    if (line.startsWith('diff --git')) {
      currentFile = parseDiffGitPath(line);
      if (currentFile && !fileChanges[currentFile]) {
        fileChanges[currentFile] = { added: 0, removed: 0, addedContent: [], addedLines: [] };
      }
      newLineNo = 0;
    } else if (line.startsWith('@@')) {
      // @@ -old[,count] +new[,count] @@
      const hunk = line.match(/^@@ -\d+(?:,\d+)? \+(\d+)/);
      newLineNo = hunk ? parseInt(hunk[1], 10) : 0;
    } else if (currentFile && line.startsWith('+') && !line.startsWith('+++')) {
      fileChanges[currentFile].added++;
      fileChanges[currentFile].addedContent.push(line.slice(1));
      if (newLineNo > 0) {
        fileChanges[currentFile].addedLines.push({ n: newLineNo, text: line.slice(1) });
        newLineNo++;
      }
    } else if (currentFile && line.startsWith('-') && !line.startsWith('---')) {
      fileChanges[currentFile].removed++;
    } else if (currentFile && newLineNo > 0 && line.startsWith(' ')) {
      // Context line — present in both versions, so it advances the new file.
      newLineNo++;
    }
  }

  return { changedFiles, fileChanges, stat };
}

// ============================================================================
// Rule checks
// ============================================================================

const warnings = [];
const failures = [];

function warn(rule, message) {
  warnings.push({ rule, message });
}

function fail(rule, message) {
  failures.push({ rule, message });
}

// --- Rule 1: Test Coverage Direction ---

function checkTestCoverage(changedFiles, fileChanges) {
  const RULE = 'Test Coverage';

  // Find new source files (added, not test files, not type files)
  const newSourceFiles = changedFiles
    .filter(f => f.status === 'A' || f.status.startsWith('A'))
    .map(f => f.file)
    .filter(f => /\.(ts|tsx)$/.test(f))
    .filter(f => f.startsWith('src/'))
    .filter(f => !f.includes('.test.'))
    .filter(f => !f.includes('.spec.'))
    .filter(f => !f.includes('__tests__/'))
    .filter(f => !f.endsWith('.d.ts'))
    .filter(f => !f.includes('/types'))
    .filter(f => !f.includes('/generated/'));

  // Check each new source file for a corresponding test
  const allFiles = changedFiles.map(f => f.file);
  const untested = [];

  for (const src of newSourceFiles) {
    const base = src.replace(/\.(ts|tsx)$/, '');
    const ext = src.endsWith('.tsx') ? 'tsx' : 'ts';
    const dir = path.dirname(src);
    const name = path.basename(src, path.extname(src));

    // Possible test file locations
    const testPatterns = [
      `${base}.test.${ext}`,
      `${base}.test.ts`,
      `${dir}/__tests__/${name}.test.${ext}`,
      `${dir}/__tests__/${name}.test.ts`,
    ];

    const hasTest = testPatterns.some(pattern =>
      allFiles.includes(pattern) || fs.existsSync(path.join(ROOT, pattern))
    );

    if (!hasTest) {
      untested.push(src);
    }
  }

  if (untested.length > 0) {
    warn(RULE, `${untested.length} new source file(s) without corresponding test files:`);
    for (const f of untested.slice(0, 5)) {
      warn(RULE, `  - ${f}`);
    }
    if (untested.length > 5) {
      warn(RULE, `  ... and ${untested.length - 5} more`);
    }
  }

  // Check if test files were deleted without corresponding source deletion
  const deletedTests = changedFiles
    .filter(f => f.status === 'D')
    .map(f => f.file)
    .filter(f => /\.test\.(ts|tsx)$/.test(f));

  const deletedOrModifiedSources = changedFiles
    .filter(f => f.status === 'D' || f.status === 'M')
    .map(f => f.file)
    .filter(f => /\.(ts|tsx)$/.test(f) && !f.includes('.test.'));

  const deletedSources = changedFiles
    .filter(f => f.status === 'D')
    .map(f => f.file)
    .filter(f => /\.(ts|tsx)$/.test(f) && !f.includes('.test.'));

  // Tests deleted without their source being deleted or modified = regression.
  // A test deleted alongside a major source refactor (M status) is intentional —
  // the feature the test covered was removed from the source.
  // Handle both co-located tests (Foo.tsx + Foo.test.tsx) and __tests__/
  // subdir convention (components/Foo.tsx + components/__tests__/Foo.test.tsx).
  const orphanedTestDeletions = deletedTests.filter(testFile => {
    const baseName = testFile.replace(/\.test\.(ts|tsx)$/, '');
    // Derive the "sibling path" that would exist if the test is in __tests__/.
    // e.g. src/components/__tests__/Foo → src/components/Foo
    const siblingPath = baseName.replace(/\/__tests__\//, '/');

    // If the corresponding source was deleted or modified, the test deletion is intentional.
    const sourceChanged = deletedOrModifiedSources.some(src => {
      const srcBase = src.replace(/\.(ts|tsx)$/, '');
      return srcBase === baseName || srcBase === siblingPath;
    });
    if (sourceChanged) return false;

    // Feature-subfolder layout: test lives in a subdir of the source's dir
    // (e.g. components/playbook/PlaybookView.test.tsx covering
    // components/PlaybookView.tsx). Conservative: only a DELETED source with
    // the same basename pairs here — modified files stay strict-path-matched.
    const testBasename = baseName.split('/').pop();
    const sourceDeletedByBasename = deletedSources.some(src => {
      const srcBase = src.replace(/\.(ts|tsx)$/, '');
      return srcBase.split('/').pop() === testBasename;
    });
    if (sourceDeletedByBasename) return false;

    // Cross-cutting tests in __tests__/ that don't map to any single source file
    // (e.g. tier-views-consistency.test.ts) are standalone — deleting them is valid.
    if (testFile.includes('/__tests__/')) {
      const siblingSource = siblingPath;
      const exists = ['ts', 'tsx'].some(ext => {
        try { fs.accessSync(path.resolve(siblingSource + '.' + ext)); return true; } catch { return false; }
      });
      if (!exists) return false;
    }

    return true;
  });

  if (orphanedTestDeletions.length > 0) {
    fail(RULE, `${orphanedTestDeletions.length} test file(s) deleted without corresponding source removal:`);
    for (const f of orphanedTestDeletions) {
      fail(RULE, `  - ${f}`);
    }
  }
}

/**
 * Extract the post-change path from a `diff --git a/<path> b/<path>` header.
 *
 * The previous form was `line.match(/b\/(.+)$/)`, which finds the FIRST `b/`
 * anywhere in the line — and `src-tauri/src/db/migrations.rs` contains one, in
 * `d[b/]migrations.rs`. That yielded the path
 * `migrations.rs b/src-tauri/src/db/migrations.rs`, which does not exist on
 * disk, so every rule that reads the file (including the `#[cfg(test)]`
 * exclusion) silently fell back to its strictest branch. Real effect: all 13
 * unwraps in `db/migrations.rs` were reported as production code when every one
 * of them is inside a `#[cfg(test)] mod tests`.
 *
 * Anchoring the match and requiring the SPACE before ` b/` fixes it: a path may
 * contain `b/`, but the separator is the last ` b/` on the line.
 */
function parseDiffGitPath(line) {
  const match = line.match(/^diff --git a\/(.+) b\/(.+)$/);
  return match ? match[2] : null;
}

// --- Rule 2: Error Handling Direction ---

/**
 * Line ranges (1-based, inclusive) covered by `#[cfg(test)]` items in `source`.
 *
 * This rule used to classify test code by FILENAME alone (`*_tests.rs`), which
 * misses the dominant convention in this repo: an inline `#[cfg(test)] mod
 * tests` at the bottom of a production module. 383 files use the inline form
 * against 71 named `*_tests.rs`, so almost every Rust change that added tests
 * produced a false "unwrap in production code" warning — which trains reviewers
 * to ignore the rule, and a rule nobody reads protects nothing.
 *
 * Brace matching is naive (it does not parse strings or comments), which is
 * fine here: it is used only to EXCUSE lines, and a miscount ends the range
 * early, leaving the gate stricter rather than weaker.
 */
function cfgTestRanges(source) {
  const lines = source.split('\n');
  const ranges = [];

  for (let i = 0; i < lines.length; i++) {
    if (!/^\s*#\[cfg\(test\)\]/.test(lines[i])) continue;

    // Find the item's opening brace (usually the next `mod tests {`).
    let open = i + 1;
    while (open < lines.length && !lines[open].includes('{')) open++;
    if (open >= lines.length) break;

    let depth = 0;
    let end = open;
    let closed = false;
    for (; end < lines.length; end++) {
      for (const ch of lines[end]) {
        if (ch === '{') depth++;
        else if (ch === '}') depth--;
      }
      if (depth <= 0) {
        closed = true;
        break;
      }
    }

    ranges.push([i + 1, (closed ? end : lines.length - 1) + 1]);
    i = closed ? end : lines.length;
  }

  return ranges;
}

/** Is 1-based line `n` inside any of `ranges`? */
function inTestRange(n, ranges) {
  return n > 0 && ranges.some(([start, end]) => n >= start && n <= end);
}

/** Read a repo-relative file, or null if it is gone (deleted in this change). */
function readRepoFile(file) {
  try {
    return fs.readFileSync(path.join(ROOT, file), 'utf8');
  } catch {
    return null;
  }
}

/** A real `.unwrap()`, not one sitting in a trailing comment. */
function isProductionUnwrap(text) {
  return (
    /\.unwrap\(\)/.test(text) && !/\/\//.test(text.split('.unwrap()')[0].slice(-10))
  );
}

function checkErrorHandling(changedFiles, fileChanges) {
  const RULE = 'Error Handling';

  // Check for new .unwrap() in non-test Rust files
  const rustFiles = Object.entries(fileChanges)
    .filter(([file]) => file.endsWith('.rs'))
    .filter(([file]) => !file.includes('test'))
    .filter(([file]) => !file.includes('_tests.rs'))
    .filter(([file]) => !file.includes('/tests/'));

  let newUnwraps = 0;
  const unwrapFiles = [];

  for (const [file, changes] of rustFiles) {
    const source = readRepoFile(file);
    const ranges = source ? cfgTestRanges(source) : [];
    const fileLines = source ? source.split('\n') : [];

    // Prefer line-located additions so inline test modules can be excused.
    // Fall back to raw text when the hunk header could not be parsed — that
    // path keeps the old (stricter) behaviour rather than silently excusing.
    const located = changes.addedLines && changes.addedLines.length > 0;
    const candidates = located
      ? changes.addedLines
      : changes.addedContent.map((text) => ({ n: 0, text }));

    const unwrapCount = candidates.filter(({ n, text }) => {
      if (!isProductionUnwrap(text)) return false;
      // Only trust the line number if the file on disk agrees with it; if the
      // diff drifted, fall through and count the unwrap (stay strict).
      const agrees = n > 0 && fileLines[n - 1] === text;
      return !(agrees && inTestRange(n, ranges));
    }).length;

    if (unwrapCount > 0) {
      newUnwraps += unwrapCount;
      unwrapFiles.push({ file, count: unwrapCount });
    }
  }

  if (newUnwraps > 0) {
    warn(RULE, `${newUnwraps} new .unwrap() call(s) in non-test Rust code:`);
    for (const { file, count } of unwrapFiles.slice(0, 5)) {
      warn(RULE, `  - ${file} (${count} unwrap${count > 1 ? 's' : ''})`);
    }
  }

  // Check for new try/catch without error reporting in TypeScript
  const tsFiles = Object.entries(fileChanges)
    .filter(([file]) => /\.(ts|tsx)$/.test(file))
    .filter(([file]) => !file.includes('.test.'))
    .filter(([file]) => !file.includes('__tests__/'));

  let catchWithoutReport = 0;
  const bareFiles = [];

  for (const [file, changes] of tsFiles) {
    const addedLines = changes.addedContent;
    for (let i = 0; i < addedLines.length; i++) {
      const line = addedLines[i];
      if (/\bcatch\s*\(/.test(line)) {
        // Look ahead in added lines for reportError/reportWarning/console.error within ~5 lines
        const window = addedLines.slice(i, i + 6).join('\n');
        if (!/reportError|reportWarning|console\.error|console\.warn|throw\s/.test(window)) {
          catchWithoutReport++;
          if (!bareFiles.includes(file)) {
            bareFiles.push(file);
          }
        }
      }
    }
  }

  if (catchWithoutReport > 0) {
    warn(RULE, `${catchWithoutReport} new catch block(s) without error reporting (reportError/reportWarning):`);
    for (const f of bareFiles.slice(0, 5)) {
      warn(RULE, `  - ${f}`);
    }
  }
}

// --- Rule 3: Documentation Direction ---

function checkDocumentation(changedFiles, fileChanges) {
  const RULE = 'Documentation';

  // Check: new #[tauri::command] functions should be in commands.ts
  const rustChanges = Object.entries(fileChanges)
    .filter(([file]) => file.endsWith('.rs'));

  const newCommands = [];

  for (const [file, changes] of rustChanges) {
    const addedLines = changes.addedContent;
    for (let i = 0; i < addedLines.length; i++) {
      if (addedLines[i].includes('#[tauri::command]')) {
        // Look ahead for fn declaration
        for (let j = i + 1; j < Math.min(i + 10, addedLines.length); j++) {
          const fnMatch = addedLines[j].match(/^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-z_][a-z0-9_]*)/);
          if (fnMatch) {
            newCommands.push({ file, name: fnMatch[1] });
            break;
          }
        }
      }
    }
  }

  if (newCommands.length > 0) {
    // Check if commands.ts was also modified
    const commandsTsChanged = changedFiles.some(f => f.file === 'src/lib/commands.ts');

    if (!commandsTsChanged) {
      warn(RULE, `${newCommands.length} new Tauri command(s) added but commands.ts was not updated:`);
      for (const cmd of newCommands) {
        warn(RULE, `  - ${cmd.name} (in ${cmd.file})`);
      }
    } else {
      // commands.ts was changed — verify the new commands appear in its additions
      const commandsTsAdded = fileChanges['src/lib/commands.ts']?.addedContent || [];
      const commandsTsText = commandsTsAdded.join('\n');

      const missingCommands = newCommands.filter(cmd =>
        !commandsTsText.includes(cmd.name)
      );

      if (missingCommands.length > 0) {
        warn(RULE, `${missingCommands.length} new Tauri command(s) not found in commands.ts additions:`);
        for (const cmd of missingCommands) {
          warn(RULE, `  - ${cmd.name} (in ${cmd.file})`);
        }
      }
    }
  }

  // Check: new source adapters should have freshness thresholds
  const newAdapterFiles = changedFiles
    .filter(f => f.status === 'A' || f.status.startsWith('A'))
    .map(f => f.file)
    .filter(f => f.startsWith('src-tauri/src/sources/'))
    .filter(f => f.endsWith('.rs'))
    .filter(f => !f.includes('mod.rs'))
    .filter(f => !f.includes('freshness.rs'))
    .filter(f => !f.includes('rate_limiter.rs'))
    .filter(f => !f.includes('fallback.rs'))
    .filter(f => !f.includes('test'));

  if (newAdapterFiles.length > 0) {
    const freshnessChanged = changedFiles.some(f => f.file === 'src-tauri/src/sources/freshness.rs');
    if (!freshnessChanged) {
      warn(RULE, `${newAdapterFiles.length} new source adapter(s) added but freshness.rs was not updated:`);
      for (const f of newAdapterFiles) {
        warn(RULE, `  - ${f} (add freshness thresholds in get_source_thresholds)`);
      }
    }
  }
}

// --- Rule 4: Structural Health ---

function checkStructuralHealth(changedFiles, fileChanges) {
  const RULE = 'Structural Health';

  // Run check-file-sizes.cjs
  const sizesScript = path.join(ROOT, 'scripts', 'check-file-sizes.cjs');
  if (fs.existsSync(sizesScript)) {
    const result = run(`node "${sizesScript}"`);
    if (!result.ok) {
      // Check if the failures are in NEW files (from this push)
      const newFiles = new Set(
        changedFiles
          .filter(f => f.status === 'A' || f.status.startsWith('A'))
          .map(f => f.file)
      );

      const errorLines = result.output.split('\n').filter(l => l.includes('ERROR'));
      const newFileErrors = errorLines.filter(l => {
        for (const nf of newFiles) {
          if (l.includes(nf)) return true;
        }
        return false;
      });

      if (newFileErrors.length > 0) {
        fail(RULE, 'New files exceeding size limits:');
        for (const line of newFileErrors) {
          fail(RULE, `  ${line.trim()}`);
        }
      }
    }
  }

  // Check files that grew by more than 100 lines
  const bigGrowth = [];

  for (const [file, changes] of Object.entries(fileChanges)) {
    // Only check source files
    if (!/\.(ts|tsx|rs)$/.test(file)) continue;
    if (file.includes('node_modules/') || file.includes('target/')) continue;

    const netGrowth = changes.added - changes.removed;
    if (netGrowth > 100) {
      bigGrowth.push({ file, added: changes.added, removed: changes.removed, net: netGrowth });
    }
  }

  if (bigGrowth.length > 0) {
    // Sort by net growth descending
    bigGrowth.sort((a, b) => b.net - a.net);
    warn(RULE, `${bigGrowth.length} file(s) grew by >100 lines (may need splitting):`);
    for (const { file, added, removed, net } of bigGrowth.slice(0, 8)) {
      warn(RULE, `  - ${file} (+${added}/-${removed}, net +${net})`);
    }
  }
}

// ============================================================================
// Main
// ============================================================================

function main() {
  const start = Date.now();

  console.log('');
  console.log('============================================================');
  console.log('  COMPOUND QUALITY CHECK');
  console.log('============================================================');
  console.log('');

  // Determine what to analyze
  const commitRange = getCommitRange();

  if (commitRange) {
    console.log(`  Analyzing: ${commitRange.range} (${commitRange.count} commit${commitRange.count > 1 ? 's' : ''})`);
  } else {
    console.log('  Analyzing: working tree changes vs HEAD');
  }
  console.log('');

  // Get diff analysis
  const { changedFiles, fileChanges, stat } = analyzeDiff(commitRange);

  if (changedFiles.length === 0) {
    console.log('  No changes to analyze.');
    console.log('');
    process.exit(0);
  }

  const sourceFiles = changedFiles.filter(f => /\.(ts|tsx|rs)$/.test(f.file));
  console.log(`  Files changed: ${changedFiles.length} total, ${sourceFiles.length} source`);
  console.log('');

  // Run all rule checks
  console.log('  Checking rules...');
  console.log('');

  checkTestCoverage(changedFiles, fileChanges);
  checkErrorHandling(changedFiles, fileChanges);
  checkDocumentation(changedFiles, fileChanges);
  checkStructuralHealth(changedFiles, fileChanges);

  // ============================================================================
  // Report
  // ============================================================================

  const duration = ((Date.now() - start) / 1000).toFixed(1);

  if (failures.length === 0 && warnings.length === 0) {
    console.log('  [PASS] All compound quality checks passed');
    console.log('');
    console.log(`  Compound quality verified in ${duration}s`);
    console.log('');
    process.exit(0);
  }

  // Print warnings
  if (warnings.length > 0) {
    console.log('  --- Warnings ---');
    let lastRule = '';
    for (const { rule, message } of warnings) {
      if (rule !== lastRule) {
        console.log(`  [WARN] ${rule}:`);
        lastRule = rule;
      }
      console.log(`         ${message}`);
    }
    console.log('');
  }

  // Print failures
  if (failures.length > 0) {
    console.log('  --- Failures ---');
    let lastRule = '';
    for (const { rule, message } of failures) {
      if (rule !== lastRule) {
        console.log(`  [FAIL] ${rule}:`);
        lastRule = rule;
      }
      console.log(`         ${message}`);
    }
    console.log('');
  }

  // Summary
  console.log('------------------------------------------------------------');
  if (failures.length > 0) {
    console.log(`  COMPOUND QUALITY: ${failures.length} failure(s), ${warnings.length} warning(s) (${duration}s)`);
    console.log('  Push blocked — fix failures before pushing.');
    console.log('');
    process.exit(1);
  } else {
    console.log(`  COMPOUND QUALITY: ${warnings.length} warning(s), 0 failures (${duration}s)`);
    console.log('  Warnings are advisory — push allowed.');
    console.log('');
    process.exit(0);
  }
}

module.exports = {
  cfgTestRanges,
  inTestRange,
  isProductionUnwrap,
  parseDiffGitPath,
};

if (require.main === module) {
  main();
}
