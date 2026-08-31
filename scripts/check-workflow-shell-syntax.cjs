#!/usr/bin/env node
/**
 * Parse-check every inline `run:` script in the GitHub workflows.
 *
 * Why this exists: on 2026-08-28 a single stray `}` in the Windows
 * "Verify Authenticode signature" step shipped to a real release tag. The YAML
 * was valid, every existing gate passed, and the error only surfaced 31
 * minutes into the release run - AFTER the build had succeeded and the 125 MB
 * installer had already uploaded - as `Unexpected token '}'`. That release
 * could not be rescued by a rerun, because a push-tag workflow is read from
 * the tag it was triggered by, not from the fixed main.
 *
 * A shell script embedded in YAML is still a shell script. Parse it.
 */
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const WORKFLOW_DIR = '.github/workflows';

// GitHub expressions are not valid syntax in any shell; swap them for an inert
// token of the same shape so the surrounding script still parses.
const stripExpr = (s) => s.replace(/\$\{\{[^}]*\}\}/g, 'GHA_EXPR');

/**
 * Extract {file, step, shell, script, line} for every inline run block.
 *
 * Shell resolution follows GitHub's own precedence: a step's own `shell:` wins,
 * otherwise the job's `defaults.run.shell`, otherwise the platform default.
 * Missing the job-level default is not a harmless gap - it makes the gate hand
 * a pwsh script to `bash -n` and report a syntax error that is not there, which
 * is the fastest way to get a gate switched off.
 */
function extractRunBlocks(file) {
  const lines = fs.readFileSync(file, 'utf8').split('\n');
  const blocks = [];
  let step = null;
  let jobShell = null;
  let inDefaults = false;
  let defaultsIndent = -1;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!line.trim()) continue;
    const indent = line.length - line.trimStart().length;

    // A new job key at indent 2 resets everything job-scoped.
    if (/^ {2}[A-Za-z_][\w-]*:\s*$/.test(line)) {
      jobShell = null;
      step = null;
      inDefaults = false;
      defaultsIndent = -1;
      continue;
    }

    // defaults: -> run: -> shell:
    if (/^\s*defaults:\s*$/.test(line)) {
      inDefaults = true;
      defaultsIndent = indent;
      continue;
    }
    if (inDefaults) {
      if (indent <= defaultsIndent) {
        inDefaults = false;
        defaultsIndent = -1;
      } else {
        const m = line.match(/^\s*shell:\s*(\S+)\s*$/);
        if (m) jobShell = m[1];
        continue;
      }
    }

    const nameM = line.match(/^(\s*)- name:\s*(.+)$/);
    if (nameM) {
      step = { indent: nameM[1].length, name: nameM[2].trim(), shell: null, line: i + 1 };
      continue;
    }
    if (!step) continue;

    const shellM = line.match(/^\s*shell:\s*(\S+)\s*$/);
    if (shellM) {
      step.shell = shellM[1];
      continue;
    }

    const runM = line.match(/^(\s*)run:\s*\|-?\s*$/);
    if (!runM) continue;

    const bodyIndent = runM[1].length + 2;
    const body = [];
    let j = i + 1;
    for (; j < lines.length; j++) {
      const l = lines[j];
      if (l.trim() === '') {
        body.push('');
        continue;
      }
      const ind = l.length - l.trimStart().length;
      if (ind < bodyIndent) break;
      body.push(l.slice(bodyIndent));
    }
    blocks.push({
      file,
      step: step.name,
      shell: step.shell || jobShell,
      shellSource: step.shell ? 'step' : jobShell ? 'job-default' : 'platform-default',
      script: body.join('\n'),
      line: i + 1,
    });
    i = j - 1;
  }
  return blocks;
}

/**
 * Resolve a PowerShell that can run the parser.
 *
 * `pwsh` (PowerShell 7) first, because it is the one that exists on all three
 * GitHub runner images - including ubuntu, where `powershell` does not exist
 * at all. Hardcoding `powershell` made every PowerShell block on Linux CI
 * report "parser unavailable" and get skipped, so the gate silently checked
 * nothing on the exact runner that gates the pull request.
 *
 * Returns null when neither is installed; callers must treat that as
 * inconclusive, never as clean.
 */
let _pwshCache;
function resolvePowerShell() {
  if (_pwshCache !== undefined) return _pwshCache;
  for (const exe of ['pwsh', 'powershell']) {
    const probe = spawnSync(exe, ['-NoProfile', '-NonInteractive', '-Command', 'exit 0'], {
      encoding: 'utf8',
    });
    if (!probe.error && probe.status === 0) {
      _pwshCache = exe;
      return _pwshCache;
    }
  }
  _pwshCache = null;
  return _pwshCache;
}

function tmpFile(ext, content) {
  const dir = process.env.TEMP || process.env.TMPDIR || '.';
  const p = path.join(dir, `wf-syntax-${process.pid}-${Math.random().toString(36).slice(2)}${ext}`);
  fs.writeFileSync(p, content);
  return p;
}

function checkPowerShell(script) {
  const exe = resolvePowerShell();
  if (!exe) return { error: new Error('no pwsh or powershell on PATH') };
  const f = tmpFile('.ps1', script);
  try {
    const lit = f.replace(/'/g, "''");
    return spawnSync(
      exe,
      [
        '-NoProfile',
        '-NonInteractive',
        '-Command',
        `$errs=$null; [void][System.Management.Automation.Language.Parser]::ParseFile('${lit}',[ref]$null,[ref]$errs); if ($errs -and $errs.Count -gt 0) { $errs | ForEach-Object { Write-Output $_.ToString() }; exit 1 }; exit 0`,
      ],
      { encoding: 'utf8' }
    );
  } finally {
    try { fs.unlinkSync(f); } catch { /* best effort */ }
  }
}

function checkBash(script) {
  const f = tmpFile('.sh', script);
  try {
    return spawnSync('bash', ['-n', f], { encoding: 'utf8' });
  } finally {
    try { fs.unlinkSync(f); } catch { /* best effort */ }
  }
}

function run(dir = WORKFLOW_DIR) {
  if (!fs.existsSync(dir)) {
    return { checked: 0, failures: [{ file: dir, step: '-', line: 0, detail: 'workflow directory missing' }], skipped: [] };
  }
  const files = fs
    .readdirSync(dir)
    .filter((f) => /\.ya?ml$/.test(f))
    .map((f) => path.join(dir, f));

  const failures = [];
  const skipped = [];
  let checked = 0;

  for (const file of files) {
    for (const b of extractRunBlocks(file)) {
      const shell = (b.shell || 'bash').toLowerCase();
      const script = stripExpr(b.script);
      if (!script.trim()) continue;

      let res;
      if (shell === 'powershell' || shell === 'pwsh') res = checkPowerShell(script);
      else if (shell === 'bash' || shell === 'sh') res = checkBash(script);
      else {
        skipped.push({ ...b, reason: `unhandled shell '${b.shell}'` });
        continue;
      }

      // A parser we could not run is INCONCLUSIVE, never a pass and never a
      // failure - saying "clean" because powershell is absent would be the
      // same false-negative this gate exists to prevent.
      if (res.error) {
        skipped.push({ ...b, reason: `parser unavailable: ${res.error.message}` });
        continue;
      }
      checked++;
      if (res.status !== 0) {
        failures.push({
          file: b.file,
          step: b.step,
          line: b.line,
          shell,
          detail: (res.stdout || res.stderr || '').trim().split('\n').slice(0, 4).join('\n    '),
        });
      }
    }
  }
  return { checked, failures, skipped };
}

if (require.main === module) {
  const dir = process.argv[2] || WORKFLOW_DIR;
  const { checked, failures, skipped } = run(dir);
  for (const s of skipped) console.log(`  skip: ${s.file} :: ${s.step} (${s.reason})`);
  if (failures.length) {
    console.error(`\nWORKFLOW SHELL SYNTAX: ${failures.length} inline script(s) do not parse\n`);
    for (const f of failures) {
      console.error(`  ${f.file}:${f.line}  [${f.shell}]\n    step: ${f.step}\n    ${f.detail}\n`);
    }
    process.exit(1);
  }
  console.log(`Workflow shell syntax: ${checked} inline script(s) parse cleanly.`);
}

module.exports = { run, extractRunBlocks, stripExpr, resolvePowerShell };
