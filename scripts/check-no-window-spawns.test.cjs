'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { analyzeSources, buildMask, programName } = require('./check-no-window-spawns.cjs');

/** Convenience: analyze a single in-memory Rust source. */
function analyze(src) {
  return analyzeSources([{ rel: 'fixture.rs', src }]);
}

test('flags a Windows-reachable spawn with no flag', () => {
  const { violations } = analyze(`
    fn run() {
        let out = Command::new("schtasks").args(["/Query"]).output();
    }
  `);
  assert.strictEqual(violations.length, 1);
  assert.match(violations[0].label, /schtasks/);
});

test('inline creation_flags clears the spawn', () => {
  const { violations, exemptions } = analyze(`
    fn run() {
        let mut cmd = Command::new("npm");
        cmd.creation_flags(0x08000000);
        cmd.output();
    }
  `);
  assert.strictEqual(violations.length, 0);
  assert.strictEqual(exemptions[0].reason, 'CREATE_NO_WINDOW applied inline');
});

test('helper-indirection clears the spawn', () => {
  const { violations, exemptions } = analyze(`
    fn suppress(cmd: &mut Command) {
        cmd.creation_flags(0x08000000);
    }
    fn run() {
        let mut cmd = Command::new("cargo");
        suppress(&mut cmd);
        cmd.output();
    }
  `);
  assert.strictEqual(violations.length, 0);
  assert.strictEqual(exemptions[0].reason, 'CREATE_NO_WINDOW applied via helper');
});

test('unix-only program is exempt without a flag', () => {
  const { violations, exemptions } = analyze(`
    fn run() {
        Command::new("pgrep").arg("securityd").output();
    }
  `);
  assert.strictEqual(violations.length, 0);
  assert.match(exemptions[0].reason, /unix-only/);
});

test('explicit marker exempts an intended window', () => {
  const { violations, exemptions } = analyze(`
    fn run() {
        // no-window-ok: user explicitly asked to open a visible terminal
        Command::new("cmd").arg("/c").arg("start").output();
    }
  `);
  assert.strictEqual(violations.length, 0);
  assert.strictEqual(exemptions[0].reason, 'explicit no-window-ok marker');
});

test('Command::new inside a string literal is NOT a spawn', () => {
  const { violations, exemptions } = analyze(`
    fn doc() {
        let s = "call Command::new(\\"git\\") to spawn"; // not real code
        let _ = s;
    }
  `);
  assert.strictEqual(violations.length, 0);
  assert.strictEqual(exemptions.length, 0);
});

test('Command::new inside a comment is NOT a spawn', () => {
  const { violations, exemptions } = analyze(`
    // historically we used Command::new("powershell") here
    fn run() {}
  `);
  assert.strictEqual(violations.length, 0);
  assert.strictEqual(exemptions.length, 0);
});

test('buildMask blanks a brace inside a char literal so spans stay balanced', () => {
  // The '{' char literal must not be counted as a real opening brace.
  const src = `fn run() { let c = '{'; Command::new("schtasks").output(); }`;
  const mask = buildMask(src);
  assert.ok(!mask.includes("'{'"), 'char-literal contents should be masked');
  const { violations } = analyze(src);
  assert.strictEqual(violations.length, 1, 'spawn still detected with char-literal brace present');
});

test('programName strips path and .exe and lowercases', () => {
  assert.strictEqual(programName('"powershell.exe"'), 'powershell');
  assert.strictEqual(programName('r"C:\\\\Windows\\\\System32\\\\where.exe"'), 'where');
  assert.strictEqual(programName('binary_path'), null, 'dynamic program returns null');
});

test('a real module-style spawn with a flag and a sibling unix branch both pass', () => {
  // Mirrors local_audit.rs: a where/which cfg split where only the whole fn carries the helper.
  const { violations } = analyze(`
    fn check() {
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("where"); c.arg("npm"); c
        } else {
            let mut c = Command::new("which"); c.arg("npm"); c
        };
        suppress_console_window(&mut cmd);
        cmd.output();
    }
    fn suppress_console_window(cmd: &mut Command) { cmd.creation_flags(0x08000000); }
  `);
  assert.strictEqual(violations.length, 0);
});
