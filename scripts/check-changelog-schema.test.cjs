// The CHANGELOG entry for the version this tree builds tells an upgrading user which database
// schema they end on. That number is TARGET_VERSION in src-tauri/src/db/migrations.rs, and it
// moves whenever a pull request adds a migration.
//
// Why this test exists: the 1.0.4 entry said "from schema 124 to 130" for ten days after a
// later change (#1131) took the schema to 131. Nothing compared the two, so the release notes
// would have named a schema the release does not produce.
//
// What it holds, for the newest entry when it is the entry for package.json's version:
// - "migrates from schema A to B": B is TARGET_VERSION, and A is the schema the previous
//   entry ended on;
// - when the schema moved since the previous entry, the sentence must be there.
'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.join(__dirname, '..');
const read = (file) => fs.readFileSync(path.join(root, file), 'utf8');

const SENTENCE = /migrates from schema (\d+) to (\d+)/;

/** Every `## [x.y.z]` entry, newest first: its version and its text. */
function entries(changelog) {
  const heads = [...changelog.matchAll(/^## \[(\d+\.\d+\.\d+)\][^\n]*$/gm)];
  return heads.map((head, i) => ({
    version: head[1],
    text: changelog.slice(head.index, i + 1 < heads.length ? heads[i + 1].index : changelog.length),
  }));
}

/** The schema an entry says the upgrade starts from and ends on, or null when it says neither. */
function statedSchemas(entryText) {
  const found = SENTENCE.exec(entryText);
  return found ? { from: Number(found[1]), to: Number(found[2]) } : null;
}

/** Problems with the newest entry, as sentences. Empty means it agrees with the tree. */
function problems({ changelog, treeVersion, targetVersion }) {
  const list = entries(changelog);
  if (list.length === 0) return ['CHANGELOG.md has no `## [x.y.z]` entry'];
  const [newest, previous] = list;
  // A tree whose version has no entry yet is another gate's business.
  if (newest.version !== treeVersion) return [];
  const stated = statedSchemas(newest.text);
  const before = previous ? statedSchemas(previous.text) : null;
  const out = [];
  if (!stated) {
    if (before && before.to !== targetVersion) {
      out.push(
        `the ${newest.version} entry does not say which schema the upgrade ends on, and the schema moved from ${before.to} to ${targetVersion} since ${previous.version}: add "migrates from schema ${before.to} to ${targetVersion}"`,
      );
    }
    return out;
  }
  if (stated.to !== targetVersion) {
    out.push(`the ${newest.version} entry says the upgrade ends on schema ${stated.to}, but this tree migrates to ${targetVersion}`);
  }
  if (before && stated.from !== before.to) {
    out.push(`the ${newest.version} entry says the upgrade starts from schema ${stated.from}, but the ${previous.version} entry ended on ${before.to}`);
  }
  return out;
}

const entry = (version, body) => `## [${version}] - 2026-01-01\n\n${body}\n\n`;
const OLDER = entry('1.0.3', 'The database migrates from schema 113 to 124 on first launch.');

test('an entry that names the schema this tree migrates to passes', () => {
  const changelog = entry('1.0.4', 'The database migrates from schema 124 to 131 on first launch.') + OLDER;
  assert.deepEqual(problems({ changelog, treeVersion: '1.0.4', targetVersion: 131 }), []);
});

test('an entry that names an older schema is refused', () => {
  const changelog = entry('1.0.4', 'The database migrates from schema 124 to 130 on first launch.') + OLDER;
  const found = problems({ changelog, treeVersion: '1.0.4', targetVersion: 131 });
  assert.equal(found.length, 1);
  assert.match(found[0], /ends on schema 130, but this tree migrates to 131/);
});

test('an entry that starts from the wrong schema is refused', () => {
  const changelog = entry('1.0.4', 'The database migrates from schema 120 to 131 on first launch.') + OLDER;
  const found = problems({ changelog, treeVersion: '1.0.4', targetVersion: 131 });
  assert.equal(found.length, 1);
  assert.match(found[0], /starts from schema 120, but the 1\.0\.3 entry ended on 124/);
});

test('a silent entry is refused once the schema has moved, and accepted when it has not', () => {
  const changelog = entry('1.0.4', 'No word about the database.') + OLDER;
  const moved = problems({ changelog, treeVersion: '1.0.4', targetVersion: 131 });
  assert.equal(moved.length, 1);
  assert.match(moved[0], /add "migrates from schema 124 to 131"/);
  assert.deepEqual(problems({ changelog, treeVersion: '1.0.4', targetVersion: 124 }), []);
});

test('a tree whose version has no entry yet is left to the release gates', () => {
  assert.deepEqual(problems({ changelog: OLDER, treeVersion: '1.0.4', targetVersion: 131 }), []);
});

test('CHANGELOG.md names the schema this tree migrates to', () => {
  const treeVersion = JSON.parse(read('package.json')).version;
  const target = /const TARGET_VERSION: i64 = (\d+);/.exec(read('src-tauri/src/db/migrations.rs'));
  assert.ok(target, 'src-tauri/src/db/migrations.rs: expected a line `const TARGET_VERSION: i64 = N;`');
  const found = problems({ changelog: read('CHANGELOG.md'), treeVersion, targetVersion: Number(target[1]) });
  assert.deepEqual(found, [], `CHANGELOG.md: ${found.join('; ')}`);
});
