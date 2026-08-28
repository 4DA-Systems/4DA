// SPDX-License-Identifier: FSL-1.1-Apache-2.0

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { EXPECTED_DESKTOP_VERSION_LINE, EXPECTED_ENDPOINT, checkReleaseChannel } = require('./check-release-channel.cjs');

const REPO_ROOT = path.resolve(__dirname, '..');
const FIXTURE_FILES = [
  'package.json',
  'src-tauri/tauri.conf.json',
  'src-tauri/Cargo.toml',
  'src-tauri/Cargo.lock',
  'src-tauri/src/db/migrations.rs',
  '.github/workflows/release.yml',
  'scripts/pin-codesigntool-sha.sh',
  '.github/workflows/build-mcpb-extensions.yml',
  'docs/NETWORK-TRANSPARENCY.md',
  'docs/SECURITY-AUDIT-GUIDE.md',
  'src-tauri/desktop-template.desktop',
];

function copyFixture() {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), '4da-release-channel-'));
  for (const file of FIXTURE_FILES) {
    const from = path.join(REPO_ROOT, file);
    const to = path.join(tmp, file);
    fs.mkdirSync(path.dirname(to), { recursive: true });
    fs.copyFileSync(from, to);
  }
  return tmp;
}

function replaceInFile(root, file, from, to) {
  const fullPath = path.join(root, file);
  const original = fs.readFileSync(fullPath, 'utf8');
  assert.ok(original.includes(from), `${file} did not include expected fixture text`);
  fs.writeFileSync(fullPath, original.replace(from, to));
}

test('current repo release channel is valid', () => {
  assert.deepEqual(checkReleaseChannel(REPO_ROOT), []);
});

test('rejects GitHub global latest as a desktop updater endpoint', () => {
  const root = copyFixture();
  replaceInFile(
    root,
    'src-tauri/tauri.conf.json',
    EXPECTED_ENDPOINT,
    'https://github.com/4DA-Systems/4DA/releases/latest/download/latest.json',
  );

  const errors = checkReleaseChannel(root);
  assert.ok(errors.some((error) => error.includes('desktop-only manifest pointer')));
  assert.ok(errors.some((error) => error.includes('global releases/latest')));
});

test('rejects accidental desktop minor bump before the 1.0 hardening line is complete', () => {
  const root = copyFixture();
  replaceInFile(root, 'package.json', '"version": "1.0.1"', '"version": "1.1.0"');
  replaceInFile(root, 'src-tauri/tauri.conf.json', '"version": "1.0.1"', '"version": "1.1.0"');
  replaceInFile(root, 'src-tauri/Cargo.toml', 'version = "1.0.1"', 'version = "1.1.0"');
  replaceInFile(
    root,
    'src-tauri/Cargo.lock',
    'name = "fourda"\nversion = "1.0.1"',
    'name = "fourda"\nversion = "1.1.0"',
  );

  const errors = checkReleaseChannel(root);
  assert.ok(errors.some((error) => error.includes(`${EXPECTED_DESKTOP_VERSION_LINE}.x hardening line`)));
});

test('rejects exact 1.0.0 once the database schema has advanced', () => {
  const root = copyFixture();
  replaceInFile(root, 'package.json', '"version": "1.0.1"', '"version": "1.0.0"');
  replaceInFile(root, 'src-tauri/tauri.conf.json', '"version": "1.0.1"', '"version": "1.0.0"');
  replaceInFile(root, 'src-tauri/Cargo.toml', 'version = "1.0.1"', 'version = "1.0.0"');
  replaceInFile(
    root,
    'src-tauri/Cargo.lock',
    'name = "fourda"\nversion = "1.0.1"',
    'name = "fourda"\nversion = "1.0.0"',
  );

  const errors = checkReleaseChannel(root);
  assert.ok(errors.some((error) => error.includes('still 1.0.0')));
  assert.ok(errors.some((error) => error.includes(`${EXPECTED_DESKTOP_VERSION_LINE}.x hardening line`)));
});

test('rejects missing latest.json as a release warning', () => {
  const root = copyFixture();
  replaceInFile(root, '.github/workflows/release.yml', '            "latest.json"\n', '');
  replaceInFile(
    root,
    '.github/workflows/release.yml',
    '          # Desirable-but-not-fatal platform coverage checks.',
    '          if ! echo "$ASSETS" | grep -qx "latest.json"; then\n            echo "::warning::latest.json (auto-updater manifest) not present - updater will be broken."\n          fi\n\n          # Desirable-but-not-fatal platform coverage checks.',
  );

  const errors = checkReleaseChannel(root);
  assert.ok(errors.some((error) => error.includes('require latest.json')));
  assert.ok(errors.some((error) => error.includes('must not downgrade')));
});

test('rejects MCP releases that can become GitHub latest', () => {
  const root = copyFixture();
  const workflow = path.join(root, '.github/workflows/build-mcpb-extensions.yml');
  fs.writeFileSync(
    workflow,
    fs.readFileSync(workflow, 'utf8').replaceAll(' \\\n              --prerelease', '').replaceAll('\n          gh release edit "$TAG" --repo "$GITHUB_REPOSITORY" --prerelease', ''),
  );

  const errors = checkReleaseChannel(root);
  assert.ok(errors.some((error) => error.includes('MCP .mcpb releases must be marked prerelease')));
  assert.ok(errors.some((error) => error.includes('Existing MCP .mcpb releases must be edited')));
});

test('rejects placeholder or unenforced CodeSignTool checksum pins', () => {
  const root = copyFixture();
  const workflow = path.join(root, '.github/workflows/release.yml');
  const original = fs.readFileSync(workflow, 'utf8');
  const currentHash = original.match(/\$expected\s*=\s*"([0-9a-fA-F]{64})"/)?.[1];
  assert.ok(currentHash, 'fixture must contain a pinned CodeSignTool SHA-256');

  fs.writeFileSync(
    workflow,
    original
      .replace(`$expected = "${currentHash}"`, '$expected = "PLACEHOLDER_SHA256_FILL_IN"')
      .replace(
        '# SHA-256 verification of the downloaded CodeSignTool archive.',
        '# SHA-256 verification of the downloaded CodeSignTool archive.\n          # TODO: compute the SHA-256 before release.',
      )
      .replace('if ($actual -ne $expected.ToLowerInvariant())', 'if ($false)'),
  );

  const errors = checkReleaseChannel(root);
  assert.ok(errors.some((error) => error.includes('placeholder CodeSignTool checksum')));
  assert.ok(errors.some((error) => error.includes('64-character SHA-256')));
  assert.ok(errors.some((error) => error.includes('fail when the downloaded CodeSignTool hash differs')));
});

test('rejects stale CodeSignTool pin helper assumptions', () => {
  const root = copyFixture();
  const helper = path.join(root, 'scripts/pin-codesigntool-sha.sh');
  fs.writeFileSync(
    helper,
    [
      '#!/usr/bin/env bash',
      'set -euo pipefail',
      'WORKFLOW=".github/workflows/release.yml"',
      'if ! grep -q "PLACEHOLDER_SHA256_FILL_IN" "$WORKFLOW"; then',
      '  echo "placeholder already replaced"',
      'fi',
      'sed -i.bak "s|PLACEHOLDER_SHA256_FILL_IN|$SHA_LC|" "$WORKFLOW"',
    ].join('\n'),
  );

  const errors = checkReleaseChannel(root);
  assert.ok(errors.some((error) => error.includes('not a placeholder checksum')));
  assert.ok(errors.some((error) => error.includes('locate the current pinned')));
  assert.ok(errors.some((error) => error.includes('replace the current pinned')));
});

// ── updater artifacts + Linux deep-link scheme ────────────────────────────
//
// These pin the two blind spots that let a four-month-old, un-updatable,
// wrong-scheme release stand: the gate checked that release.yml ASKED for
// latest.json, but never that Tauri was configured to PRODUCE it, and nothing
// in the repo referenced desktop-template.desktop at all.

test('a missing createUpdaterArtifacts fails the gate', () => {
  const root = copyFixture();
  const file = 'src-tauri/tauri.conf.json';
  const cfg = JSON.parse(fs.readFileSync(path.join(root, file), 'utf8'));
  delete cfg.bundle.createUpdaterArtifacts;
  fs.writeFileSync(path.join(root, file), JSON.stringify(cfg, null, 2));
  const failures = checkReleaseChannel(root);
  assert.ok(
    failures.some((f) => f.includes('createUpdaterArtifacts')),
    `expected a createUpdaterArtifacts failure, got: ${JSON.stringify(failures)}`
  );
});

test('createUpdaterArtifacts set to false fails the gate', () => {
  const root = copyFixture();
  const file = 'src-tauri/tauri.conf.json';
  const cfg = JSON.parse(fs.readFileSync(path.join(root, file), 'utf8'));
  cfg.bundle.createUpdaterArtifacts = false;
  fs.writeFileSync(path.join(root, file), JSON.stringify(cfg, null, 2));
  assert.ok(checkReleaseChannel(root).some((f) => f.includes('createUpdaterArtifacts')));
});

test('a desktop entry registering a retired scheme fails the gate', () => {
  const root = copyFixture();
  replaceInFile(
    root,
    'src-tauri/desktop-template.desktop',
    'MimeType=x-scheme-handler/fourda;',
    'MimeType=x-scheme-handler/4da;'
  );
  const failures = checkReleaseChannel(root);
  // Both directions should fire: the configured scheme is missing, and a
  // scheme that is not configured is registered.
  assert.ok(failures.some((f) => f.includes('x-scheme-handler/fourda')));
  assert.ok(failures.some((f) => f.includes('not in')));
});

test('a desktop entry that drops %U fails the gate', () => {
  const root = copyFixture();
  replaceInFile(root, 'src-tauri/desktop-template.desktop', 'Exec={{exec}} %U', 'Exec={{exec}}');
  assert.ok(checkReleaseChannel(root).some((f) => f.includes('%U')));
});

// ── dry-run tag safety ────────────────────────────────────────────────────
//
// The two publishing steps were unconditional. Eight `v0.0.N-test` dry-run tags
// exist in this repo's history; every one failed earlier in the matrix, which is
// the only reason a dry run never clobbered the production updater pointer and
// offered a test build to installed clients as an update.

test('an unguarded updater-pointer publish fails the gate', () => {
  const root = copyFixture();
  replaceInFile(
    root,
    '.github/workflows/release.yml',
    "      - name: Publish desktop updater manifest\n        if: needs.create-release.outputs.production == 'true'\n",
    '      - name: Publish desktop updater manifest\n'
  );
  const failures = checkReleaseChannel(root);
  assert.ok(
    failures.some((f) => f.includes('Publish desktop updater manifest') && f.includes('dry-run')),
    `expected an unguarded-pointer failure, got: ${JSON.stringify(failures)}`
  );
});

test('an unguarded un-draft step fails the gate', () => {
  const root = copyFixture();
  replaceInFile(
    root,
    '.github/workflows/release.yml',
    "      - name: Publish release\n        if: needs.create-release.outputs.production == 'true'\n",
    '      - name: Publish release\n'
  );
  assert.ok(
    checkReleaseChannel(root).some((f) => f.includes('Publish release') && f.includes('dry-run'))
  );
});

test('removing the tag classifier fails the gate', () => {
  const root = copyFixture();
  replaceInFile(root, '.github/workflows/release.yml', 'id: tagkind', 'id: something-else');
  assert.ok(checkReleaseChannel(root).some((f) => f.includes('classify the tag')));
});

test('a loosened tag pattern that would accept a -test tag fails the gate', () => {
  // The whole point is that `v0.0.14-test` must NOT classify as production.
  // A pattern without the `$` anchor would let it through.
  const root = copyFixture();
  replaceInFile(
    root,
    '.github/workflows/release.yml',
    String.raw`^v[0-9]+\.[0-9]+\.[0-9]+$`,
    String.raw`^v[0-9]+\.[0-9]+\.[0-9]+`
  );
  assert.ok(
    checkReleaseChannel(root).some((f) => f.includes('vMAJOR.MINOR.PATCH')),
    'a prefix-only tag pattern must be rejected'
  );
});

test('an ungated publish-mcp job fails the gate', () => {
  // npm publishes cannot be undone. This job had no tag classification at all.
  const root = copyFixture();
  replaceInFile(
    root,
    '.github/workflows/release.yml',
    "    if: needs.create-release.outputs.production == 'true'\n    runs-on: ubuntu-latest\n    timeout-minutes: 20",
    '    runs-on: ubuntu-latest\n    timeout-minutes: 20'
  );
  assert.ok(
    checkReleaseChannel(root).some((f) => f.includes('publish-mcp') && f.includes('cannot be undone')),
    'an ungated npm publish must fail the gate'
  );
});

// ── Windows signing waiver (ALLOW_UNSIGNED_WINDOWS) ───────────────────────
//
// This is the one deliberate hole in a gate that exists because unsigned
// builds once shipped silently under a green tick. A green run proves nothing
// about a hole; these three tests are what make it safe.

test('a BOOLEAN waiver fails the gate — it must be scoped to one tag', () => {
  // A boolean left set to true authorises every future release. A tag name
  // cannot, because the next release has a different tag.
  const root = copyFixture();
  replaceInFile(
    root,
    '.github/workflows/release.yml',
    '[ "${ALLOW_UNSIGNED_WINDOWS}" = "$TAG" ]',
    '[ "${ALLOW_UNSIGNED_WINDOWS}" = "true" ]'
  );
  assert.ok(
    checkReleaseChannel(root).some((f) => f.includes('against the tag being built')),
    'a boolean waiver must be rejected'
  );
});

test('extending the waiver to macOS fails the gate', () => {
  // An un-notarized .app is REFUSED by Gatekeeper, not warned about. Waiving
  // it ships a macOS build that cannot be opened at all.
  const root = copyFixture();
  replaceInFile(
    root,
    '.github/workflows/release.yml',
    '              require APPLE_CERTIFICATE',
    '              if [ "${UNSIGNED_WINDOWS_OK:-}" != "true" ]; then require APPLE_CERTIFICATE; fi'
  );
  assert.ok(
    checkReleaseChannel(root).some((f) => f.includes('Gatekeeper')),
    'a macOS waiver must be rejected'
  );
});

test('skipping the signature CHECK under the waiver fails the gate', () => {
  // The waiver may change the verdict. It must never stop the measurement —
  // that distinction is the entire subject of #437.
  const root = copyFixture();
  replaceInFile(
    root,
    '.github/workflows/release.yml',
    "        if: runner.os == 'Windows'\n        env:\n          ALLOW_UNSIGNED_WINDOWS:",
    "        if: runner.os == 'Windows' && needs.create-release.outputs.unsigned_windows_ok != 'true'\n        env:\n          ALLOW_UNSIGNED_WINDOWS:"
  );
  assert.ok(
    checkReleaseChannel(root).some((f) => f.includes('unconditional')),
    'making the check conditional on the waiver must be rejected'
  );
});

test('the current workflow, with the waiver present, passes all three', () => {
  assert.deepEqual(checkReleaseChannel(REPO_ROOT), []);
});
