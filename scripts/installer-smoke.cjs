#!/usr/bin/env node
/**
 * installer-smoke.cjs: the two halves of .github/workflows/installer-smoke.yml
 * that are worth unit-testing.
 *
 *   fetch  Find a release by tag (published or DRAFT), pick this platform's
 *          installer by its SHAPE, download it and check its SHA-256 against
 *          the release's SHASUMS256.txt (and the per-file .sha256 sidecar, when
 *          the release has one). Writes `installer`, `name` and `draft` to
 *          $GITHUB_OUTPUT.
 *
 *   watch  After the workflow has launched the installed app on a fresh
 *          profile: wait for the readiness line in the app's own log, then
 *          require the process to stay alive for a while, with no fatal line
 *          in the log at any point. Exit 1 with a `::error::` annotation on any
 *          miss.
 *
 *   resolve  Which release to test and which to upgrade from: the given tags,
 *          else the newest PUBLISHED production release (vX.Y.Z, not draft,
 *          not prerelease) and the newest one that is not the tag under test.
 *          Writes `tag` and `previous_tag` to $GITHUB_OUTPUT.
 *
 * Usage:
 *   node scripts/installer-smoke.cjs resolve --repo OWNER/NAME [--tag T] [--previous-tag P]
 *   node scripts/installer-smoke.cjs fetch --repo OWNER/NAME --tag vX.Y.Z \
 *        --kind win|macArm|macIntel|linuxDeb|linuxAppImage --dir <download dir>
 *   node scripts/installer-smoke.cjs watch --log-dir <dir> --pid <pid> \
 *        [--ready <text>] [--budget-secs 120] [--alive-secs 45] [--fatal <text>]... \
 *        [--page <text>] [--output-file <launcher output>] [--title <text>] \
 *        [--summary-match <regex>]
 *
 * Asset names are never templated: Tauri derives them from productName, which
 * has changed before (see update-site-release.cjs). The shapes come from that
 * script so the download page and this check cannot disagree.
 */
'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { SAFE_ASSET_NAME, shapes, selectInstallers } = require('./update-site-release.cjs');

const SHASUMS = 'SHASUMS256.txt';

// The last line `setup_app` writes once the essential services are up. In a
// release build "Main window shown" is logged at the TOP of setup_app
// (start_frontend_readiness_gate marks first light as soon as the AppHandle
// exists), so this later line proves strictly more.
const DEFAULT_READY = 'setup_app: phase 0 (essential services) complete';

// A line that means the app is up but broken. "Panic detected" is the panic
// hook (crash_guard.rs); the others are the startup paths that log and carry on.
const DEFAULT_FATAL = [
  'Panic detected',
  'Database initialization failed',
  'Context Engine initialization failed',
  'FATAL:',
];

// Logged when the notification page's own JavaScript emits `notification-ready`:
// the bundled frontend ran in the installed webview and reached the backend.
// Evidence, not a requirement: up to v1.0.3 the listener is registered after
// the window is created, so the event can be lost on a healthy start.
const DEFAULT_PAGE = 'Notification window JS listener registered';

// ---------------------------------------------------------------------------
// Pure logic (unit-tested in installer-smoke.test.cjs)
// ---------------------------------------------------------------------------

/**
 * The release whose tag is exactly `tag`, from a REST "list releases" result.
 * The list endpoint is used on purpose: "get a release by tag name" returns
 * published releases only, while the list includes drafts for a token with
 * push access (GITHUB_TOKEN with `contents: write`).
 */
function findRelease(releases, tag) {
  const hits = releases.filter((r) => r.tag_name === tag);
  if (hits.length === 0) {
    throw new Error(
      `no release with tag ${tag} is visible to this token. A DRAFT release is listed only ` +
        'for a token with push access (GITHUB_TOKEN needs `contents: write`).',
    );
  }
  if (hits.length > 1) {
    const ids = hits.map((r) => `${r.id}${r.draft ? ' (draft)' : ''}`).join(', ');
    throw new Error(`${hits.length} releases carry tag ${tag} (${ids}); delete the stale one first`);
  }
  return hits[0];
}

const PRODUCTION_TAG = /^v(\d+)\.(\d+)\.(\d+)$/;

/** Published production releases (vX.Y.Z, not draft, not prerelease), newest first. */
function publishedProduction(releases) {
  const key = (t) => PRODUCTION_TAG.exec(t).slice(1).map(Number);
  return releases
    .filter((r) => !r.draft && !r.prerelease && PRODUCTION_TAG.test(r.tag_name))
    .sort((a, b) => {
      const [x, y] = [key(a.tag_name), key(b.tag_name)];
      return y[0] - x[0] || y[1] - x[1] || y[2] - x[2];
    });
}

/**
 * The tag to test and the tag to upgrade from. A given tag wins; otherwise the
 * newest published production release, so pull-request runs follow the
 * newest release a user can actually download. `previous` is empty when there
 * is nothing else published.
 */
function resolveTags(releases, { tag = '', previous = '' } = {}) {
  const prod = publishedProduction(releases);
  const resolvedTag = tag || (prod[0] && prod[0].tag_name);
  if (!resolvedTag) throw new Error('no published production release (vX.Y.Z) to test');
  const other = prod.find((r) => r.tag_name !== resolvedTag);
  return { tag: resolvedTag, previous: previous || (other ? other.tag_name : '') };
}

/** This platform's installer: exactly one asset of the right shape, safe name. */
function pickInstaller(assets, kind) {
  const pred = shapes[kind];
  if (!pred) throw new Error(`unknown kind ${JSON.stringify(kind)}; expected one of ${Object.keys(shapes).join(', ')}`);
  const hits = selectInstallers(assets).filter((a) => pred(a.name));
  if (hits.length === 0) throw new Error(`no ${kind} installer among the release assets`);
  if (hits.length > 1) throw new Error(`ambiguous ${kind} installer: ${hits.map((h) => h.name).join(', ')}`);
  if (!SAFE_ASSET_NAME.test(hits[0].name)) throw new Error(`unsafe asset name ${JSON.stringify(hits[0].name)}`);
  return hits[0];
}

/** Find an asset by exact name, or null. */
function assetNamed(assets, name) {
  return assets.find((a) => a.name === name) || null;
}

/**
 * Parse `sha256sum` output: "<64 hex>  <name>" (or " *<name>"). Any other line
 * is an error, not skipped: a checksum file we only half understand must not
 * be able to vouch for anything.
 */
function parseShasums(text, label = SHASUMS) {
  const sums = new Map();
  text.split(/\r?\n/).forEach((raw, i) => {
    if (!raw.trim()) return;
    const m = /^([0-9a-fA-F]{64}) [ *](\S+)\s*$/.exec(raw);
    if (!m) throw new Error(`${label} line ${i + 1} is not "<sha256>  <file>": ${JSON.stringify(raw.slice(0, 120))}`);
    const hash = m[1].toLowerCase();
    if (sums.has(m[2]) && sums.get(m[2]) !== hash) throw new Error(`${label} lists ${m[2]} twice with different hashes`);
    sums.set(m[2], hash);
  });
  if (sums.size === 0) throw new Error(`${label} lists no files`);
  return sums;
}

/** Throw unless `actual` is the hash `sums` lists for `name`. */
function checkHash(sums, name, actual, label = SHASUMS) {
  const expected = sums.get(name);
  if (!expected) throw new Error(`${label} has no entry for ${name}`);
  if (expected !== actual.toLowerCase()) {
    throw new Error(`SHA-256 mismatch for ${name}: ${label} says ${expected}, the download is ${actual}`);
  }
}

function sha256File(file) {
  const hash = crypto.createHash('sha256');
  const fd = fs.openSync(file, 'r');
  const buf = Buffer.alloc(1 << 20);
  try {
    let n;
    while ((n = fs.readSync(fd, buf, 0, buf.length, null)) > 0) hash.update(buf.subarray(0, n));
  } finally {
    fs.closeSync(fd);
  }
  return hash.digest('hex');
}

/** First log line containing `ready`, and first containing any fatal text. */
function scanLog(text, ready, fatal) {
  let readyLine = null;
  let fatalLine = null;
  for (const line of text.split(/\r?\n/)) {
    if (readyLine === null && line.includes(ready)) readyLine = line;
    if (fatalLine === null && fatal.some((f) => line.includes(f))) fatalLine = line;
  }
  return { readyLine, fatalLine };
}

/**
 * One tick of the watch. `readyAtMs` is when the readiness line was first
 * seen (ms since launch), or null. Returns { status: 'waiting' | 'pass' |
 * 'fail', reason }.
 */
function verdict({ scan, alive, elapsedMs, readyAtMs, budgetMs, aliveMs }) {
  const secs = (ms) => `${Math.round(ms / 1000)} s`;
  if (scan.fatalLine) return { status: 'fail', reason: `the app logged a fatal line: ${scan.fatalLine.trim()}` };
  if (readyAtMs === null) {
    if (!alive) return { status: 'fail', reason: `the app exited ${secs(elapsedMs)} after launch, before its readiness line` };
    if (elapsedMs > budgetMs) return { status: 'fail', reason: `no readiness line in the app log within ${secs(budgetMs)}` };
    return { status: 'waiting' };
  }
  const up = elapsedMs - readyAtMs;
  if (!alive) return { status: 'fail', reason: `the app exited ${secs(up)} after its readiness line (required: ${secs(aliveMs)})` };
  if (up >= aliveMs) return { status: 'pass', reason: `ready after ${secs(readyAtMs)}, still alive ${secs(up)} later` };
  return { status: 'waiting' };
}

/** Up to `max` log lines matching `pattern` (a regular expression), trimmed. */
function matchingLines(text, pattern, max = 40) {
  const re = new RegExp(pattern);
  return text.split(/\r?\n/).filter((l) => re.test(l)).slice(0, max).map((l) => l.trim());
}

/** Escape a value for a `::error::` workflow command. */
function annotation(text) {
  return String(text).replace(/%/g, '%25').replace(/\r/g, '%0D').replace(/\n/g, '%0A');
}

// ---------------------------------------------------------------------------
// I/O
// ---------------------------------------------------------------------------

function parseArgs(argv) {
  const out = { _: [], fatal: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith('--')) { out._.push(a); continue; }
    const key = a.slice(2);
    const val = argv[i + 1];
    if (val === undefined || val.startsWith('--')) throw new Error(`${a} needs a value`);
    i++;
    if (key === 'fatal') out.fatal.push(val);
    else out[key] = val;
  }
  return out;
}

function required(args, ...keys) {
  for (const k of keys) if (!args[k]) throw new Error(`--${k} is required`);
}

function gh(args, opts = {}) {
  const r = spawnSync('gh', args, { encoding: 'utf8', maxBuffer: 64 << 20, windowsHide: true, ...opts });
  if (r.error) throw new Error(`could not run gh: ${r.error.message}`);
  if (r.status !== 0) throw new Error(`gh ${args[0]} ${args[args.length - 1]} failed (exit ${r.status}): ${String(r.stderr || '').trim().split('\n')[0]}`);
  return r.stdout;
}

/** Stream one release asset (by id, so drafts work) to `dir`; check its size. */
function downloadAsset(repo, asset, dir) {
  const dest = path.join(dir, asset.name);
  const fd = fs.openSync(dest, 'w');
  try {
    gh(['api', '-H', 'Accept: application/octet-stream', `repos/${repo}/releases/assets/${asset.id}`], {
      stdio: ['ignore', fd, 'pipe'],
      encoding: 'buffer',
      timeout: 20 * 60 * 1000,
    });
  } finally {
    fs.closeSync(fd);
  }
  const size = fs.statSync(dest).size;
  if (size !== asset.size) throw new Error(`${asset.name}: downloaded ${size} bytes, the release lists ${asset.size}`);
  return dest;
}

function setOutput(values) {
  const lines = Object.entries(values).map(([k, v]) => `${k}=${v}`);
  for (const l of lines) console.log(`  output ${l}`);
  if (process.env.GITHUB_OUTPUT) fs.appendFileSync(process.env.GITHUB_OUTPUT, lines.join('\n') + '\n');
}

/** Every release this token can see (drafts only with push access). */
function listReleases(repo) {
  return JSON.parse(gh(['api', '--paginate', '--slurp', `repos/${repo}/releases?per_page=100`])).flat();
}

function resolveCommand(args) {
  required(args, 'repo');
  const { tag, previous } = resolveTags(listReleases(args.repo), { tag: args.tag, previous: args['previous-tag'] });
  console.log(`testing ${tag}; upgrade legs start from ${previous || '(no other published release)'}`);
  setOutput({ tag, previous_tag: previous });
}

function fetchCommand(args) {
  required(args, 'repo', 'tag', 'kind', 'dir');
  const release = findRelease(listReleases(args.repo), args.tag);
  console.log(`${args.tag}: release ${release.id}${release.draft ? ' (DRAFT)' : ''}, ${release.assets.length} assets`);

  const installer = pickInstaller(release.assets, args.kind);
  const sumsAsset = assetNamed(release.assets, SHASUMS);
  if (!sumsAsset) throw new Error(`${args.tag} has no ${SHASUMS}; nothing to check the download against`);

  fs.mkdirSync(args.dir, { recursive: true });
  const sums = parseShasums(fs.readFileSync(downloadAsset(args.repo, sumsAsset, args.dir), 'utf8'));
  console.log(`downloading ${installer.name} (${Math.round(installer.size / 1048576)} MB)`);
  const file = downloadAsset(args.repo, installer, args.dir);
  const actual = sha256File(file);
  checkHash(sums, installer.name, actual);
  console.log(`  SHA-256 ${actual} matches ${SHASUMS}`);

  const sidecar = assetNamed(release.assets, `${installer.name}.sha256`);
  if (sidecar) {
    const label = sidecar.name;
    checkHash(parseShasums(fs.readFileSync(downloadAsset(args.repo, sidecar, args.dir), 'utf8'), label), installer.name, actual, label);
    console.log(`  and ${label} agrees`);
  }
  setOutput({ installer: file, name: installer.name, draft: String(Boolean(release.draft)) });
}

/** Every regular file in the log dir, in name order (the appender rolls daily). */
function readLogs(dir) {
  let names;
  try {
    names = fs.readdirSync(dir).sort();
  } catch {
    return '';
  }
  return names
    .map((n) => path.join(dir, n))
    .filter((p) => { try { return fs.statSync(p).isFile(); } catch { return false; } })
    .map((p) => { try { return fs.readFileSync(p, 'utf8'); } catch { return ''; } })
    .join('\n');
}

function isAlive(pid) {
  if (process.platform === 'linux') {
    // kill(pid, 0) succeeds on a zombie; /proc tells a dead child from a live one.
    try {
      const stat = fs.readFileSync(`/proc/${pid}/stat`, 'utf8');
      const state = stat.slice(stat.lastIndexOf(')') + 2).charAt(0);
      return state !== 'Z' && state !== 'X';
    } catch {
      return false;
    }
  }
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return e.code === 'EPERM';
  }
}

/** Print text the app wrote without letting a `::command::` in it reach the runner. */
function printUntrusted(title, text) {
  const token = crypto.randomBytes(16).toString('hex');
  console.log(`::group::${title}`);
  console.log(`::stop-commands::${token}`);
  console.log(text);
  console.log(`::${token}::`);
  console.log('::endgroup::');
}

async function watchCommand(args) {
  required(args, 'log-dir', 'pid');
  const pid = Number(args.pid);
  if (!Number.isInteger(pid) || pid <= 0) throw new Error(`--pid must be a process id, got ${args.pid}`);
  const ready = args.ready || DEFAULT_READY;
  const fatal = args.fatal.length ? args.fatal : DEFAULT_FATAL;
  const page = args.page || DEFAULT_PAGE;
  const budgetMs = Number(args['budget-secs'] || 120) * 1000;
  const aliveMs = Number(args['alive-secs'] || 45) * 1000;
  const title = args.title || 'Installer smoke';
  console.log(`watching pid ${pid}; log dir ${args['log-dir']}`);
  console.log(`readiness line: "${ready}" within ${budgetMs / 1000} s, then alive for ${aliveMs / 1000} s`);

  const started = Date.now();
  let readyAtMs = null;
  let readyLine = null;
  for (;;) {
    const text = readLogs(args['log-dir']);
    const scan = scanLog(text, ready, fatal);
    const alive = isAlive(pid);
    const elapsedMs = Date.now() - started;
    if (scan.readyLine && readyAtMs === null) {
      readyAtMs = elapsedMs;
      readyLine = scan.readyLine.trim();
      console.log(`readiness line after ${(elapsedMs / 1000).toFixed(1)} s:`);
      printUntrusted('Readiness line', readyLine);
    }
    const v = verdict({ scan, alive, elapsedMs, readyAtMs, budgetMs, aliveMs });
    if (v.status !== 'waiting') {
      const lines = text.split(/\r?\n/);
      printUntrusted(`App log (last 80 of ${lines.length} lines)`, lines.slice(-80).join('\n'));
      const summary = process.env.GITHUB_STEP_SUMMARY;
      const evidence = args['summary-match'] ? matchingLines(text, args['summary-match']) : [];
      const evidenceMd = evidence.length
        ? `\nLog lines matching \`${args['summary-match'].replace(/`/g, '')}\`:\n\n\`\`\`\n${evidence.join('\n').replace(/`/g, "'")}\n\`\`\`\n`
        : '';
      if (evidence.length) printUntrusted(`Log lines matching ${args['summary-match']}`, evidence.join('\n'));
      if (v.status === 'pass') {
        const pageLine = scanLog(text, page, []).readyLine;
        console.log(`PASS: ${v.reason}`);
        if (pageLine) printUntrusted('The page ran JavaScript', pageLine.trim());
        else console.log(`::warning title=${annotation(title)}::no "${annotation(page)}" line, so nothing shows the page ran JavaScript`);
        if (summary) fs.appendFileSync(summary, `### ${title}: PASS\n\n${v.reason}.\n\nReadiness line:\n\n\`\`\`\n${readyLine}\n\`\`\`\n${evidenceMd}`);
        return 0;
      }
      // A panic's message reaches stderr, not the log, in builds up to v1.0.3.
      if (args['output-file']) {
        let out = '';
        try { out = fs.readFileSync(args['output-file'], 'utf8'); } catch { out = '(not readable)'; }
        printUntrusted(`Launcher output (last 40 lines of ${args['output-file']})`, out.split(/\r?\n/).slice(-40).join('\n'));
      }
      console.log(`::error title=${annotation(title)}::${annotation(v.reason)}`);
      if (summary) fs.appendFileSync(summary, `### ${title}: FAIL\n\n${v.reason}\n${evidenceMd}`);
      return 1;
    }
    await new Promise((r) => setTimeout(r, 1000));
  }
}

async function main(argv) {
  const args = parseArgs(argv);
  const cmd = args._[0];
  if (cmd === 'resolve') { resolveCommand(args); return 0; }
  if (cmd === 'fetch') { fetchCommand(args); return 0; }
  if (cmd === 'watch') return watchCommand(args);
  throw new Error('usage: installer-smoke.cjs resolve|fetch|watch [options] (see the file header)');
}

module.exports = {
  DEFAULT_READY,
  DEFAULT_FATAL,
  findRelease,
  resolveTags,
  matchingLines,
  pickInstaller,
  parseShasums,
  checkHash,
  sha256File,
  scanLog,
  verdict,
  annotation,
  parseArgs,
};

if (require.main === module) {
  main(process.argv.slice(2)).then(
    (code) => process.exit(code),
    (e) => {
      console.log(`::error title=Installer smoke::${annotation(e.message)}`);
      process.exit(1);
    },
  );
}
