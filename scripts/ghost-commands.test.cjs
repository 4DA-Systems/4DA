// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Tests for the ghost-command pre-commit gate (scripts/ghost-commands.cjs),
// which blocks Tauri commands that are registered in generate_handler![] but
// that no frontend code ever calls.
//
// Run: node --test scripts/ghost-commands.test.cjs   (or `pnpm run test:scripts`)
//
// THE CENTRAL CASE (see the first test): a command with a `CommandMap` key but
// no call site MUST be classified as a ghost.
//
// That is the exact case that regressed. The gate used to treat every
// CommandMap interface key in src/lib/commands.ts as evidence of USE — while a
// second blocking gate (scripts/validate-commands.cjs) *enforces* that every
// registered command HAS a CommandMap key. The two gates formed a closed loop:
// gate B manufactured the artifact gate A accepted as proof, so gate A could
// never fail. It reported "0 ghosts / IPC health 100%" while ~121 of 404
// registered commands (30% of the IPC surface) were unreachable, and persisted
// that false 100% to .claude/wisdom/ghost-commands.json.
//
// Every test here runs against a synthetic fixture repo in os.tmpdir(), so the
// assertions stay true as the real command set changes.

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { analyze } = require('./ghost-commands.cjs');

// ---------------------------------------------------------------------------
// Fixture builder
// ---------------------------------------------------------------------------

const write = (file, contents) => {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, contents, 'utf8');
};

/**
 * Build a miniature 4DA repo.
 *
 * @param {object} opts
 * @param {string} opts.rust           contents of src-tauri/src/mod_a.rs
 * @param {string} opts.handler        body of generate_handler![ ... ]
 * @param {string} opts.commandsTs     contents of src/lib/commands.ts
 * @param {string} [opts.component]    contents of src/components/Thing.tsx
 * @param {string} [opts.publicJs]     contents of public/briefing.js
 * @param {string} [opts.e2e]          contents of e2e/smoke.spec.ts
 * @param {object[]} [opts.backlog]    ghost-command-backlog.json entries
 * @returns {{ root: string, backlogPath: string }}
 */
function fixture(opts) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), '4da-ghost-test-'));
  write(path.join(root, 'src-tauri', 'src', 'mod_a.rs'), opts.rust);
  if (opts.publicJs !== undefined) write(path.join(root, 'public', 'briefing.js'), opts.publicJs);
  if (opts.e2e !== undefined) write(path.join(root, 'e2e', 'smoke.spec.ts'), opts.e2e);
  write(
    path.join(root, 'src-tauri', 'src', 'lib.rs'),
    `pub fn run() {\n  tauri::Builder::default()\n    .invoke_handler(tauri::generate_handler![\n${opts.handler}\n    ])\n    .run(ctx)\n}\n`,
  );
  write(path.join(root, 'src', 'lib', 'commands.ts'), opts.commandsTs);
  if (opts.component !== undefined) {
    write(path.join(root, 'src', 'components', 'Thing.tsx'), opts.component);
  }
  const backlogPath = path.join(root, 'ghost-command-backlog.json');
  fs.writeFileSync(backlogPath, JSON.stringify({ backlog: opts.backlog || [] }), 'utf8');
  return { root, backlogPath };
}

const run = (opts) => {
  const { root, backlogPath } = fixture(opts);
  try {
    return analyze({ root, backlogPath });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
};

const names = (list) => list.map((c) => c.name).sort();

// A CommandMap declaring both commands — exactly what validate-commands.cjs
// forces to exist for every registered command.
const COMMAND_MAP = `
export interface CommandMap {
  called_cmd: { params: never; result: string };
  typed_but_uncalled: { params: never; result: string };
}
export function cmd(command, params) {
  return invoke(command, params ?? {});
}
`;

const TWO_COMMANDS = `
#[tauri::command]
pub async fn called_cmd() -> Result<String> { Ok(String::new()) }

#[tauri::command]
pub async fn typed_but_uncalled() -> Result<String> { Ok(String::new()) }
`;

const TWO_REGISTERED = '      mod_a::called_cmd,\n      mod_a::typed_but_uncalled,';

// ---------------------------------------------------------------------------
// THE REGRESSION
// ---------------------------------------------------------------------------

test('CENTRAL CASE: a command with a CommandMap key but NO call site is a GHOST', () => {
  const r = run({
    rust: TWO_COMMANDS,
    handler: TWO_REGISTERED,
    commandsTs: COMMAND_MAP,
    component: `import { cmd } from '../lib/commands';\nexport const load = () => cmd('called_cmd');\n`,
  });

  assert.deepEqual(names(r.ghosts), ['typed_but_uncalled'],
    'a CommandMap key is a TYPE DECLARATION, never evidence of use');
  assert.deepEqual(names(r.liveDirect), ['called_cmd']);
  assert.equal(r.total, 2);
  assert.equal(r.ipcHealthPct, 50, 'ipc_health_pct must be live/total, not type coverage');
  assert.equal(r.typeCoveragePct, 100, 'type coverage stays 100% — and must not rescue the ghost');
});

test('the two metrics are independent: full type coverage does NOT imply IPC health', () => {
  const r = run({
    rust: TWO_COMMANDS,
    handler: TWO_REGISTERED,
    commandsTs: COMMAND_MAP,
    component: undefined, // nothing calls anything
  });
  assert.equal(r.typeCoveragePct, 100);
  assert.equal(r.ipcHealthPct, 0);
  assert.equal(r.ghosts.length, 2);
});

test('a name listed in a commands.ts helper Set is NOT a call site', () => {
  // src/lib/commands.ts holds sets like LONG_RUNNING_COMMANDS that enumerate
  // command names. commands.ts is excluded from the name-literal scan for
  // exactly this reason.
  const r = run({
    rust: TWO_COMMANDS,
    handler: TWO_REGISTERED,
    commandsTs: `${COMMAND_MAP}\nconst LONG_RUNNING_COMMANDS = new Set(['typed_but_uncalled']);\n`,
    component: `import { cmd } from '../lib/commands';\nexport const load = () => cmd('called_cmd');\n`,
  });
  assert.deepEqual(names(r.ghosts), ['typed_but_uncalled']);
});

// ---------------------------------------------------------------------------
// Real call sites
// ---------------------------------------------------------------------------

test('LIVE: direct cmd() and invoke() call sites, with and without type args', () => {
  const r = run({
    rust: TWO_COMMANDS,
    handler: TWO_REGISTERED,
    commandsTs: COMMAND_MAP,
    component: `cmd<string>('called_cmd');\ninvoke<string>("typed_but_uncalled");\n`,
  });
  assert.equal(r.ghosts.length, 0);
  assert.equal(r.liveDirect.length, 2);
  assert.equal(r.ipcHealthPct, 100);
});

test('LIVE (indirect): a bare name literal outside commands.ts counts as dynamic dispatch', () => {
  // Mirrors src/components/settings/useSourceConfig.ts, which passes the command
  // name to a helper that calls cmd(cmdName).
  const r = run({
    rust: TWO_COMMANDS,
    handler: TWO_REGISTERED,
    commandsTs: COMMAND_MAP,
    component: `cmd('called_cmd');\nmakeToggle(setState, 'typed_but_uncalled', 'feeds');\n`,
  });
  assert.equal(r.ghosts.length, 0);
  assert.deepEqual(names(r.liveIndirect), ['typed_but_uncalled']);
  assert.deepEqual(names(r.liveDirect), ['called_cmd']);
});

test('LIVE: public/ webviews count, and invoke wrappers are followed', () => {
  // public/briefing.js and public/notification.js are shipped app surfaces that
  // reach the backend through a wrapper: invokeTauri('briefing_open_url', ...).
  // A literal /invoke\(/ pattern misses the wrapper and the whole directory.
  const r = run({
    rust: TWO_COMMANDS,
    handler: TWO_REGISTERED,
    commandsTs: COMMAND_MAP,
    component: `cmd('called_cmd');\n`,
    publicJs: `openBtn.addEventListener('click', function () {\n  invokeTauri('typed_but_uncalled', {});\n});\n`,
  });
  assert.equal(r.ghosts.length, 0, 'a command called from public/ is not a ghost');
  assert.deepEqual(names(r.liveDirect).sort(), ['called_cmd', 'typed_but_uncalled']);
});

test('e2e specs do NOT count as usage — a test-only command is still a ghost', () => {
  // Counting tests as usage would recreate the "the artifact proves itself"
  // loop that broke this gate in the first place.
  const r = run({
    rust: TWO_COMMANDS,
    handler: TWO_REGISTERED,
    commandsTs: COMMAND_MAP,
    component: `cmd('called_cmd');\n`,
    e2e: `test('x', async () => { await page.evaluate(() => invoke('typed_but_uncalled')); });\n`,
  });
  assert.deepEqual(names(r.ghosts), ['typed_but_uncalled']);
});

// ---------------------------------------------------------------------------
// Parser correctness (the denominator)
// ---------------------------------------------------------------------------

test('DENOMINATOR: pub(crate) commands are detected', () => {
  // The old fn regex required `pub ` and so silently skipped every
  // `pub(crate) fn` command — 15 of them in the real tree, never classified.
  const r = run({
    rust: `
#[tauri::command]
pub(crate) async fn called_cmd() -> Result<String> { Ok(String::new()) }

#[tauri::command]
pub(crate) fn typed_but_uncalled() -> Result<String> { Ok(String::new()) }
`,
    handler: TWO_REGISTERED,
    commandsTs: COMMAND_MAP,
    component: `cmd('called_cmd');\n`,
  });
  assert.equal(r.total, 2, 'pub(crate) commands must count toward the denominator');
  assert.deepEqual(names(r.ghosts), ['typed_but_uncalled']);
});

test('DENOMINATOR: #[tauri::command(...)] attribute forms are detected', () => {
  const r = run({
    rust: `
#[tauri::command(rename_all = "snake_case")]
pub async fn called_cmd() -> Result<String> { Ok(String::new()) }

#[tauri::command]
#[allow(dead_code)] // REMOVE BY 2099-01-01
pub async fn typed_but_uncalled() -> Result<String> { Ok(String::new()) }
`,
    handler: TWO_REGISTERED,
    commandsTs: COMMAND_MAP,
    component: `cmd('called_cmd');\n`,
  });
  assert.equal(r.total, 2, 'attribute args and interleaved attributes must not hide a command');
});

test('DENOMINATOR: comment words inside generate_handler![] are not registrations', () => {
  // The old registration regex ran over raw text and harvested `first`,
  // `reader` and `summaries` out of a comment, inflating the count by 3.
  const r = run({
    rust: TWO_COMMANDS,
    handler: `      // Content (article reader, AI summaries, saved items)\n${TWO_REGISTERED}`,
    commandsTs: COMMAND_MAP,
    component: `cmd('called_cmd');\n`,
  });
  assert.equal(r.registrations, 2, 'only module::function entries count');
});

test('UNREGISTERED: a Rust command missing from generate_handler![] is flagged', () => {
  const r = run({
    rust: TWO_COMMANDS,
    handler: '      mod_a::called_cmd,',
    commandsTs: COMMAND_MAP,
    component: `cmd('called_cmd');\n`,
  });
  assert.deepEqual(names(r.unregistered), ['typed_but_uncalled']);
  assert.equal(r.ghosts.length, 0, 'unregistered is its own bucket, not a ghost');
});

// ---------------------------------------------------------------------------
// Backlog allowlist — the ratchet
// ---------------------------------------------------------------------------

test('BACKLOG: an allowlisted ghost is reported but does not block', () => {
  const r = run({
    rust: TWO_COMMANDS,
    handler: TWO_REGISTERED,
    commandsTs: COMMAND_MAP,
    component: `cmd('called_cmd');\n`,
    backlog: [{ command: 'typed_but_uncalled', since: '2026-08-14', reason: 'pre-existing' }],
  });
  assert.equal(r.ghosts.length, 0, 'allowlisted ghosts must not fail the build');
  assert.deepEqual(names(r.backlogged), ['typed_but_uncalled']);
  assert.equal(r.ipcHealthPct, 50, 'allowlisting must NOT flatter the health metric');
});

test('BACKLOG: a NEW ghost still blocks while the backlog is non-empty', () => {
  const r = run({
    rust: `${TWO_COMMANDS}\n#[tauri::command]\npub async fn brand_new_ghost() -> Result<String> { Ok(String::new()) }\n`,
    handler: `${TWO_REGISTERED}\n      mod_a::brand_new_ghost,`,
    commandsTs: `${COMMAND_MAP}\n// brand_new_ghost: { params: never; result: string };\n`,
    component: `cmd('called_cmd');\n`,
    backlog: [{ command: 'typed_but_uncalled', since: '2026-08-14', reason: 'pre-existing' }],
  });
  assert.deepEqual(names(r.ghosts), ['brand_new_ghost'], 'day-one regression blocking');
});

test('BACKLOG: a stale entry (command now called) is reported so the list shrinks', () => {
  const r = run({
    rust: TWO_COMMANDS,
    handler: TWO_REGISTERED,
    commandsTs: COMMAND_MAP,
    component: `cmd('called_cmd');\ncmd('typed_but_uncalled');\n`,
    backlog: [{ command: 'typed_but_uncalled', since: '2026-08-14', reason: 'pre-existing' }],
  });
  assert.equal(r.staleBacklog.length, 1);
  assert.match(r.staleBacklog[0].why, /now has a frontend caller/);
});

test('BACKLOG: a stale entry (command deleted from Rust) is reported', () => {
  const r = run({
    rust: TWO_COMMANDS,
    handler: TWO_REGISTERED,
    commandsTs: COMMAND_MAP,
    component: `cmd('called_cmd');\ncmd('typed_but_uncalled');\n`,
    backlog: [{ command: 'deleted_long_ago', since: '2026-08-14', reason: 'pre-existing' }],
  });
  assert.equal(r.staleBacklog.length, 1);
  assert.match(r.staleBacklog[0].why, /no longer exists/);
});

// ---------------------------------------------------------------------------
// The shipped allowlist is honest about the real tree
// ---------------------------------------------------------------------------

test('the shipped backlog file parses and every entry is dated', () => {
  const raw = JSON.parse(
    fs.readFileSync(path.join(__dirname, 'ghost-command-backlog.json'), 'utf8'),
  );
  assert.ok(Array.isArray(raw.backlog));
  for (const entry of raw.backlog) {
    assert.match(entry.command, /^[a-z_][a-z0-9_]*$/);
    assert.match(entry.since, /^\d{4}-\d{2}-\d{2}$/, `${entry.command} needs a since date`);
    assert.ok(entry.reason && entry.reason.length > 0, `${entry.command} needs a reason`);
  }
  const unique = new Set(raw.backlog.map((e) => e.command));
  assert.equal(unique.size, raw.backlog.length, 'no duplicate backlog entries');
});
