#!/usr/bin/env node
/*
 * check-no-window-spawns.cjs — build gate: no spawned child process may flash a console window.
 *
 * WHY THIS EXISTS
 *   fourda.exe is a GUI-subsystem binary (windows_subsystem = "windows"), so it owns no console.
 *   On Windows, when a console-subsystem child is spawned WITHOUT the CREATE_NO_WINDOW flag, the OS
 *   allocates a brand-new console for it — a black window paints on the user's desktop, often only
 *   for a flicker. At scale (every install, every spawn) that reads as malware and erodes trust. The
 *   founder's literal first instinct on seeing one was to kill the process.
 *
 *   The fix is per-spawn discipline: every std/tokio `Command` that can run on Windows must set
 *       cmd.creation_flags(0x08000000) // CREATE_NO_WINDOW   (via std::os::windows::process::CommandExt)
 *   This gate makes that discipline enforced instead of remembered, so the next `Command::new`
 *   someone adds cannot regress it.
 *
 * WHAT IT CHECKS
 *   For every `Command::new(...)` under src-tauri/src (excluding tests and the deliberately-console
 *   `fourda-engine` binary), the gate requires ONE of:
 *     (a) the enclosing function also calls `.creation_flags(` — the CREATE_NO_WINDOW idiom; or
 *     (b) the program is a known Unix-only binary (ps, sh, codesign, ...) that never runs on Windows; or
 *     (c) an explicit `// no-window-ok: <reason>` marker on the spawn line or the line above it.
 *   Anything else is a violation (exit 1).
 *
 * It is deliberately conservative: an unrecognized spawn must PROVE it is safe (flag, allowlisted
 * binary, or justified marker) rather than be assumed safe.
 */
'use strict';

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const SRC_DIR = path.join(ROOT, 'src-tauri', 'src');

// Unix-only programs: these are gated to macOS/Linux and can never spawn on Windows, so they cannot
// flash a Windows console. Keep this list tight — only genuinely non-Windows tools belong here.
const UNIX_ONLY = new Set([
  'ps', 'sh', 'df', 'kill', 'killall', 'pgrep', 'pkill', 'fc-list', 'codesign', 'defaults',
  'ldconfig', 'lspci', 'lsusb', 'which', 'uname', 'sysctl', 'sw_vers', 'system_profiler',
  'securityd', 'open', 'xdg-open', 'lsb_release', 'dpkg', 'rpm', 'ioreg', 'pmset', 'diskutil',
  'launchctl', 'osascript', 'plutil', 'scutil',
]);

const EXEMPT_MARKER = 'no-window-ok';

// Files to skip: test files (window flash is irrelevant in tests) and the fourda-engine binary,
// which is intentionally a console-subsystem binary (it hides its own console at runtime via
// hide_scheduler_spawned_console(); see headless.rs).
function isSkippedFile(rel) {
  if (/(^|[\\/])tests?[\\/]/.test(rel)) return true;
  if (/_tests?\.rs$/.test(rel)) return true;
  if (rel.replace(/\\/g, '/').endsWith('src/bin/fourda-engine.rs')) return true;
  return false;
}

/** Recursively collect .rs files under a directory. */
function collectRustFiles(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) collectRustFiles(full, out);
    else if (entry.name.endsWith('.rs')) out.push(full);
  }
  return out;
}

/*
 * Build a "mask" of the source the same length as the original, with the CONTENTS of comments,
 * strings, raw strings and char literals replaced by spaces. Token searches and brace matching run
 * against the mask so we never match `Command::new` inside a string or miscount a `{` inside `'{'`.
 * The original text is kept for reading program-name literals.
 */
function buildMask(src) {
  const mask = new Array(src.length);
  let i = 0;
  const n = src.length;
  const space = (from, to) => { for (let k = from; k < to; k++) mask[k] = ' '; };

  while (i < n) {
    const c = src[i];
    const c2 = src[i + 1];

    // Line comment
    if (c === '/' && c2 === '/') {
      let j = i;
      while (j < n && src[j] !== '\n') j++;
      space(i, j);
      i = j;
      continue;
    }
    // Block comment (Rust block comments nest)
    if (c === '/' && c2 === '*') {
      let depth = 1;
      let j = i + 2;
      while (j < n && depth > 0) {
        if (src[j] === '/' && src[j + 1] === '*') { depth++; j += 2; }
        else if (src[j] === '*' && src[j + 1] === '/') { depth--; j += 2; }
        else j++;
      }
      space(i, j);
      i = j;
      continue;
    }
    // Raw string: r"...", r#"..."#, br##"..."##
    if ((c === 'r' || c === 'b') && /[rb]*#*"/.test(src.slice(i, i + 6))) {
      // find the opening sequence: optional b/r prefix, then # run, then "
      let j = i;
      while (j < n && (src[j] === 'r' || src[j] === 'b')) j++;
      if (src[j] === '#' || src[j] === '"') {
        let hashes = 0;
        while (src[j] === '#') { hashes++; j++; }
        if (src[j] === '"') {
          j++; // past opening quote
          const closer = '"' + '#'.repeat(hashes);
          const end = src.indexOf(closer, j);
          const stop = end === -1 ? n : end + closer.length;
          space(i, stop);
          i = stop;
          continue;
        }
      }
      // not actually a raw string (e.g. identifier starting with r/b) — fall through
    }
    // Normal string
    if (c === '"') {
      let j = i + 1;
      while (j < n) {
        if (src[j] === '\\') { j += 2; continue; }
        if (src[j] === '"') { j++; break; }
        j++;
      }
      space(i, j);
      i = j;
      continue;
    }
    // Char literal vs lifetime: 'a' / '\n' / '{' are literals; 'static is a lifetime.
    if (c === "'") {
      if (c2 === '\\') {
        // escaped char literal: '\n', '\'', '\x41', '\u{1F}'
        let j = i + 2;
        while (j < n && src[j] !== "'") j++;
        space(i, j + 1);
        i = j + 1;
        continue;
      }
      if (src[i + 2] === "'") {
        // simple char literal 'x'
        space(i, i + 3);
        i += 3;
        continue;
      }
      // lifetime tick — emit as-is and move on
      mask[i] = c;
      i++;
      continue;
    }
    mask[i] = c;
    i++;
  }
  return mask.join('');
}

/** Find [bodyStart, bodyEnd) brace spans of every `fn` in the masked source, with the fn name. */
function findFunctionSpans(mask) {
  const spans = [];
  const fnRe = /\bfn\s+(\w+)/g;
  let m;
  while ((m = fnRe.exec(mask)) !== null) {
    const open = mask.indexOf('{', m.index);
    if (open === -1) continue;
    let depth = 0;
    let j = open;
    for (; j < mask.length; j++) {
      if (mask[j] === '{') depth++;
      else if (mask[j] === '}') { depth--; if (depth === 0) { j++; break; } }
    }
    spans.push({ start: open, end: j, name: m[1] });
  }
  return spans;
}

/** Smallest function span containing index, or null. */
function enclosingSpan(spans, idx) {
  let best = null;
  for (const s of spans) {
    if (idx >= s.start && idx < s.end) {
      if (!best || (s.end - s.start) < (best.end - best.start)) best = s;
    }
  }
  return best;
}

/** Read the argument text of Command::new( ... ) starting at the '(' index in the mask. */
function readCallArg(src, mask, parenIdx) {
  let depth = 0;
  let j = parenIdx;
  for (; j < mask.length; j++) {
    if (mask[j] === '(') depth++;
    else if (mask[j] === ')') { depth--; if (depth === 0) break; }
  }
  return src.slice(parenIdx + 1, j);
}

/** Extract a normalized program name from a literal arg, or null if the arg is dynamic. */
function programName(argText) {
  const t = argText.trim();
  const lit = t.match(/^(?:r#*)?"((?:[^"\\]|\\.)*)"/) || t.match(/^"([^"]*)"/);
  if (!lit) return null; // variable / expression
  let name = lit[1];
  name = name.split(/[\\/]/).pop().toLowerCase();
  name = name.replace(/\.exe$/, '');
  return name;
}

function lineNumberOf(src, idx) {
  let line = 1;
  for (let k = 0; k < idx; k++) if (src[k] === '\n') line++;
  return line;
}

function lineTextAt(src, lineNo) {
  return src.split('\n')[lineNo - 1] || '';
}

/**
 * Analyze a set of { rel, src } Rust sources for spawns that could flash a Windows console.
 * Pure (no filesystem) so it can be unit-tested. Returns { violations, exemptions }.
 */
function analyzeSources(sources) {
  const violations = [];
  const exemptions = [];

  // ---- Pass 1: collect the names of "protective helper" functions ----
  // A protective helper is any function whose body applies `.creation_flags(`. A spawn that hands
  // its Command to such a helper (e.g. `suppress_console_window(&mut cmd)`) is just as protected as
  // one that sets the flag inline, so the gate must treat a call to one as equivalent.
  const parsed = [];
  const protectiveHelpers = new Set();
  for (const { rel, src } of sources) {
    const mask = buildMask(src);
    const spans = findFunctionSpans(mask);
    for (const s of spans) {
      if (mask.slice(s.start, s.end).includes('creation_flags(')) protectiveHelpers.add(s.name);
    }
    if (src.includes('Command::new')) parsed.push({ rel, src, mask, spans });
  }

  const helperCallRe = protectiveHelpers.size
    ? new RegExp(`\\b(?:${[...protectiveHelpers].join('|')})\\s*\\(`)
    : null;

  // ---- Pass 2: evaluate every Command::new site ----
  for (const { rel, src, mask, spans } of parsed) {
    const callRe = /Command::new\s*\(/g;
    let m;
    while ((m = callRe.exec(mask)) !== null) {
      const parenIdx = mask.indexOf('(', m.index);
      const idx = m.index;
      const lineNo = lineNumberOf(src, idx);
      const arg = readCallArg(src, mask, parenIdx);
      const prog = programName(arg);
      const fnSpan = enclosingSpan(spans, idx);
      const fnText = fnSpan ? mask.slice(fnSpan.start, fnSpan.end) : mask;

      const lineText = lineTextAt(src, lineNo);
      const prevText = lineTextAt(src, lineNo - 1);
      const marker = lineText.includes(EXEMPT_MARKER) || prevText.includes(EXEMPT_MARKER);

      let reason = null;
      if (fnText.includes('creation_flags(')) reason = 'CREATE_NO_WINDOW applied inline';
      else if (helperCallRe && helperCallRe.test(fnText)) reason = 'CREATE_NO_WINDOW applied via helper';
      else if (prog && UNIX_ONLY.has(prog)) reason = `unix-only program "${prog}"`;
      else if (marker) reason = 'explicit no-window-ok marker';

      const label = prog ? `Command::new("${prog}")` : `Command::new(${arg.trim().slice(0, 40)})`;
      if (reason) exemptions.push({ rel, lineNo, label, reason });
      else violations.push({ rel, lineNo, label });
    }
  }
  return { violations, exemptions };
}

/** CLI entry point. Returns the process exit code. */
function main(argv) {
  const files = collectRustFiles(SRC_DIR).filter((f) => !isSkippedFile(path.relative(ROOT, f)));
  const sources = files.map((f) => ({
    rel: path.relative(ROOT, f).replace(/\\/g, '/'),
    src: fs.readFileSync(f, 'utf8'),
  }));
  const { violations, exemptions } = analyzeSources(sources);

  console.log(`no-window spawn gate: scanned ${files.length} files, ` +
    `${exemptions.length + violations.length} Command::new sites ` +
    `(${exemptions.length} safe, ${violations.length} violation${violations.length === 1 ? '' : 's'}).`);

  if (argv.includes('--verbose')) {
    for (const e of exemptions) console.log(`  ok   ${e.rel}:${e.lineNo}  ${e.label}  — ${e.reason}`);
  }

  if (violations.length) {
    console.error('\nA spawned child process may flash a console window on Windows:\n');
    for (const v of violations) console.error(`  ${v.rel}:${v.lineNo}  ${v.label}`);
    console.error('\nFix each by ONE of:');
    console.error('  • add `cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW` (import');
    console.error('    std::os::windows::process::CommandExt) inside the function, OR');
    console.error('  • if the program is genuinely Unix-only, add it to UNIX_ONLY in this script, OR');
    console.error('  • add `// no-window-ok: <reason>` on the spawn line if a window is intended.');
    return 1;
  }
  return 0;
}

module.exports = { analyzeSources, buildMask, findFunctionSpans, programName };

if (require.main === module) {
  process.exit(main(process.argv));
}
